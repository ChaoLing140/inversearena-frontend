import { Contract } from "@stellar/stellar-sdk";
import { Server } from "@stellar/stellar-sdk/rpc";

export type SorobanServerConstructor = new (serverUrl: string) => Server;

export type ContractClientFactoryDeps = {
  Server: SorobanServerConstructor;
};

/**
 * Creates Soroban RPC clients and {@link Contract} handles without embedding URLs in call sites.
 * Inject `deps.Server` in tests to avoid real RPC.
 *
 * Implements singleton pattern for Server and contract cache to reduce
 * re-initialization overhead on hot paths.
 */
export class ContractClientFactory {
  private _rpcServer: Server | null = null;
  private _contractCache = new Map<string, Contract>();

  constructor(
    private readonly sorobanRpcUrl: string,
    private readonly deps: ContractClientFactoryDeps = { Server },
  ) {}

  get rpcUrl(): string {
    return this.sorobanRpcUrl;
  }

  createRpcServer(): Server {
    if (!this._rpcServer) {
      this._rpcServer = new this.deps.Server(this.sorobanRpcUrl);
    }
    return this._rpcServer;
  }

  createContract(contractId: string): Contract {
    let contract = this._contractCache.get(contractId);
    if (!contract) {
      contract = new Contract(contractId);
      this._contractCache.set(contractId, contract);
    }
    return contract;
  }

  clearCache(): void {
    this._rpcServer = null;
    this._contractCache.clear();
  }
}
