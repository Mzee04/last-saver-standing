import { WalletAdapterNetwork } from "@solana/wallet-adapter-base";
import { ConnectionProvider, WalletProvider } from "@solana/wallet-adapter-react";
import { WalletModalProvider } from "@solana/wallet-adapter-react-ui";
import { PhantomWalletAdapter, SolflareWalletAdapter } from "@solana/wallet-adapter-wallets";
import { clusterApiUrl } from "@solana/web3.js";
import { StrictMode, useMemo, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "@solana/wallet-adapter-react-ui/styles.css";
import "./styles.css";

const rpc = import.meta.env.VITE_SOLANA_RPC ?? clusterApiUrl("devnet");
const cluster = import.meta.env.VITE_SOLANA_CLUSTER ?? "devnet";

function Root() {
  const wallets = useMemo(
    () => [
      new PhantomWalletAdapter(),
      new SolflareWalletAdapter({ network: WalletAdapterNetwork.Devnet }),
    ],
    [],
  );
  // Wallet-adapter ships its own @types/react@19, which disagrees with this app's React 18 types.
  const Providers = ConnectionProvider as unknown as (props: {
    endpoint: string;
    children?: ReactNode;
  }) => ReactNode;
  const Wallets = WalletProvider as unknown as (props: {
    wallets: typeof wallets;
    autoConnect?: boolean;
    children?: ReactNode;
  }) => ReactNode;
  const Modal = WalletModalProvider as unknown as (props: { children?: ReactNode }) => ReactNode;
  return (
    <Providers endpoint={rpc}>
      <Wallets wallets={wallets} autoConnect>
        <Modal>
          <App endpoint={rpc} cluster={cluster} />
        </Modal>
      </Wallets>
    </Providers>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);
