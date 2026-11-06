import { queryOptions } from "@tanstack/react-query";
import { adultApi } from "../../api/adult";
import { providerApi } from "../../api/providers";
import { providerAuthApi } from "../../api/provider-auth";

export const providersQuery = (adult = false) => queryOptions({
  queryKey: adult ? ["adult", "providers"] : ["providers"],
  queryFn: () => adult ? adultApi.providers() : providerApi.snapshot(),
});

export const providerAuthQuery = (providerId: string, adult = false) => queryOptions({
  queryKey: adult ? ["adult", "provider-auth", providerId] : ["provider-auth", providerId],
  queryFn: () => providerAuthApi.status(providerId),
});
