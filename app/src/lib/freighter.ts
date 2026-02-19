export interface WalletSession {
  address: string;
  signMessage: (message: string) => Promise<string>;
}

type FreighterAddressResponse =
  | string
  | {
      address?: string;
      publicKey?: string;
      error?: string;
    };

type FreighterSignResponse =
  | string
  | {
      signature?: string;
      signedMessage?: string;
      signed_message?: string;
      error?: string;
    };

type FreighterApi = {
  requestAccess?: () => Promise<unknown>;
  getAddress?: () => Promise<FreighterAddressResponse>;
  signMessage?: (
    message: string,
    opts?: { address?: string }
  ) => Promise<FreighterSignResponse>;
};

declare global {
  interface Window {
    freighterApi?: FreighterApi;
  }
}

function getFreighterApi(): FreighterApi {
  if (typeof window === "undefined" || !window.freighterApi) {
    throw new Error("Freighter wallet not found");
  }
  return window.freighterApi;
}

function parseAddress(result: FreighterAddressResponse): string {
  if (typeof result === "string") {
    return result;
  }
  if (result.error) {
    throw new Error(result.error);
  }
  if (typeof result.address === "string") {
    return result.address;
  }
  if (typeof result.publicKey === "string") {
    return result.publicKey;
  }
  throw new Error("Freighter returned an invalid address response");
}

function parseSignature(result: FreighterSignResponse): string {
  if (typeof result === "string") {
    return result;
  }
  if (result.error) {
    throw new Error(result.error);
  }
  if (typeof result.signature === "string") {
    return result.signature;
  }
  if (typeof result.signedMessage === "string") {
    return result.signedMessage;
  }
  if (typeof result.signed_message === "string") {
    return result.signed_message;
  }
  throw new Error("Freighter returned an invalid signature response");
}

export async function connectFreighterWallet(): Promise<WalletSession> {
  const api = getFreighterApi();

  if (api.requestAccess) {
    await api.requestAccess();
  }

  if (!api.getAddress) {
    throw new Error("Freighter getAddress API is unavailable");
  }

  const address = parseAddress(await api.getAddress());

  if (!api.signMessage) {
    throw new Error("Freighter signMessage API is unavailable");
  }

  const signMessage = async (message: string): Promise<string> => {
    const sig = await api.signMessage!(message, { address });
    return parseSignature(sig);
  };

  return {
    address,
    signMessage,
  };
}
