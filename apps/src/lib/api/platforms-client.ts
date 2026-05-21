import { invoke, withAddr } from "./transport";

export const platformsClient = {
  async discover(params?: { addr?: string }): Promise<unknown> {
    return invoke("service_platforms_discovery", withAddr(params ?? {}));
  },
};
