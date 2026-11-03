use mediaatlas_domain::ProviderFailure;
use reqwest::{
    dns::{Addrs, Name, Resolve, Resolving},
    Url,
};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113)
                || a >= 224)
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            // Only global unicast 2000::/3, excluding reserved transition and docs.
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}
pub fn safe_origin(url: &str) -> String {
    Url::parse(url)
        .ok()
        .filter(|u| u.has_host() && matches!(u.scheme(), "http" | "https"))
        .map(|u| u.origin().ascii_serialization())
        .unwrap_or_else(|| "[redacted]".to_owned())
}

#[derive(Clone, Default)]
pub(crate) struct DestinationPolicy {
    #[cfg(test)]
    pub(crate) loopback: bool,
}
impl DestinationPolicy {
    pub(crate) fn validate_url(&self, url: &Url) -> Result<(), ProviderFailure> {
        #[cfg(test)]
        if self.loopback
            && matches!(url.scheme(), "http" | "https")
            && url.host_str() == Some("127.0.0.1")
            && url.username().is_empty()
            && url.password().is_none()
        {
            return Ok(());
        }
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.host_str().is_none()
        {
            return Err(ProviderFailure::UnsafeTarget);
        }
        let host = url
            .host_str()
            .unwrap_or_default()
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if host == "localhost"
            || host.ends_with(".localhost")
            || host.ends_with(".local")
            || host.ends_with(".internal")
            || host == "metadata.google.internal"
            || !host.contains('.') && !host.contains(':')
        {
            return Err(ProviderFailure::UnsafeTarget);
        }
        if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
            if !is_public_ip(ip) {
                return Err(ProviderFailure::UnsafeTarget);
            }
        }
        Ok(())
    }
    pub(crate) async fn resolve(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Vec<SocketAddr>, ProviderFailure> {
        let addrs = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| ProviderFailure::DnsError)?
            .collect::<Vec<_>>();
        if addrs.is_empty() {
            return Err(ProviderFailure::DnsError);
        }
        for addr in &addrs {
            #[cfg(test)]
            if self.loopback && addr.ip() == IpAddr::from([127, 0, 0, 1]) {
                continue;
            }
            if !is_public_ip(addr.ip()) {
                return Err(ProviderFailure::UnsafeTarget);
            }
        }
        Ok(addrs)
    }
}
/// The resolver returns the exact vetted addresses that the connector will use;
/// it does not perform a separate check followed by a second untrusted lookup.
pub(crate) struct SafeResolver(pub(crate) Arc<DestinationPolicy>);
impl Resolve for SafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let policy = self.0.clone();
        Box::pin(async move {
            let addrs = policy
                .resolve(name.as_str(), 0)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}
