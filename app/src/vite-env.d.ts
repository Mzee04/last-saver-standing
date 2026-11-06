/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_SOLANA_RPC?: string;
  readonly VITE_SOLANA_CLUSTER?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
