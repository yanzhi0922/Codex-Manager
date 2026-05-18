import type { Metadata } from "next";
import "./globals.css";
import { Sidebar } from "@/components/layout/sidebar";
import { Header } from "@/components/layout/header";
import { DesktopPageViewport } from "@/components/layout/desktop-page-viewport";
import { RouteTransitionOverlay } from "@/components/layout/route-transition-overlay";
import { Providers } from "@/components/providers";
import { AppBootstrap } from "@/components/layout/app-bootstrap";
import {
  appearanceInitScript,
  DEFAULT_APPEARANCE_PRESET,
} from "@/lib/appearance";

export const metadata: Metadata = {
  title: "Codex-Copilot",
  description: "Unified account pool, gateway, and session management for Codex",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="zh-CN"
      suppressHydrationWarning
      data-appearance={DEFAULT_APPEARANCE_PRESET}
    >
      <body className="antialiased">
        <script dangerouslySetInnerHTML={{ __html: appearanceInitScript }} />
        <Providers>
          <AppBootstrap>
            <div className="flex h-screen overflow-hidden">
              <Sidebar />
              <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
                <Header />
                <main className="relative min-w-0 flex-1 overflow-y-auto p-6 no-scrollbar">
                  <RouteTransitionOverlay />
                  <DesktopPageViewport>{children}</DesktopPageViewport>
                </main>
              </div>
            </div>
          </AppBootstrap>
        </Providers>
      </body>
    </html>
  );
}
