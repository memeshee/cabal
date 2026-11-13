import type { Metadata, Viewport } from "next";
import { Big_Shoulders, IBM_Plex_Mono, Schibsted_Grotesk, Special_Elite } from "next/font/google";
import "./globals.css";

const display = Big_Shoulders({ subsets: ["latin"], weight: ["700", "800"], variable: "--font-display", display: "swap" });
const body = Schibsted_Grotesk({ subsets: ["latin"], variable: "--font-body", display: "swap" });
const mono = IBM_Plex_Mono({ subsets: ["latin"], weight: ["400", "500", "600", "700"], variable: "--font-mono", display: "swap" });
const stamp = Special_Elite({ subsets: ["latin"], weight: "400", variable: "--font-stamp", display: "swap" });

export const metadata: Metadata = {
  title: "CABAL: find the insider before the dump",
  description: "A memecoin party game on NEAR. One KOL at your table is dumping on you through fresh wallets. The chain shows who. People lie; the chain doesn't.",
  icons: { icon: "/icon.svg", shortcut: "/icon.svg" },
  openGraph: { title: "CABAL", description: "One KOL at the table is dumping on you. Catch them from the on-chain evidence.", type: "website" },
};
export const viewport: Viewport = { themeColor: "#14110d" };

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${display.variable} ${body.variable} ${mono.variable} ${stamp.variable}`}>
      <body>{children}</body>
    </html>
  );
}
