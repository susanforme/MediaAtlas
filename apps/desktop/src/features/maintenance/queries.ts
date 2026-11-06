import { queryOptions } from "@tanstack/react-query";
import { databaseMaintenanceApi } from "../../api/maintenance";

export const maintenanceStatusQuery = queryOptions({ queryKey: ["database-maintenance"], queryFn: databaseMaintenanceApi.status });
