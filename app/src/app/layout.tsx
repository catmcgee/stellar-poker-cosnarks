import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Poker on Stellar",
  description: "Onchain poker with private cards via MPC + ZK proofs on Stellar",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
