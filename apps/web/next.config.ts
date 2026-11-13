import path from "node:path";
import type { NextConfig } from "next";

// Where the Rust house runs. Baked into the rewrite at build time (the container sets it to localhost).
const API = process.env.API_URL ?? "http://127.0.0.1:8080";

const config: NextConfig = {
  reactStrictMode: true,
  output: "standalone",
  outputFileTracingRoot: path.join(__dirname, "../.."),
  poweredByHeader: false,
  async rewrites() {
    return [{ source: "/api/:path*", destination: `${API}/api/:path*` }];
  },
};
export default config;
