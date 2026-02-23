import "@/css/satoshi.css";
import "@/css/style.css";

import { Providers } from "./providers";
import { Sidebar } from "@/components/layouts/sidebar";
import { ContentWrapper } from "@/components/layouts/content-wrapper";
import { TransparencyDetector } from "@/components/transparency-detector";
import { BackgroundOpacitySync } from "@/components/background-opacity-sync";
import { ThemeSync } from "@/components/theme-sync";

export const metadata = {
  title: "Nayru",
  description: "Text-to-speech reader",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html
      lang="en"
      className="dark transparent-mode"
      style={{ backgroundColor: "rgba(9, 9, 11, 0.55)" }}
    >
      <body>
        <Providers>
          <TransparencyDetector />
          <BackgroundOpacitySync />
          <ThemeSync />
          <div className="flex min-h-screen">
            <Sidebar />
            <ContentWrapper>
              <main className="relative w-full flex-1 overflow-hidden">
                {children}
              </main>
            </ContentWrapper>
          </div>
        </Providers>
      </body>
    </html>
  );
}
