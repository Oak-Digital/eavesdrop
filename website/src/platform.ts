type PlatformNavigator = Pick<
  Navigator,
  "platform" | "userAgent" | "maxTouchPoints"
> & {
  userAgentData?: { platform?: string; mobile?: boolean };
};

export function detectPlatform(
  browser: PlatformNavigator | undefined = typeof navigator === "undefined"
    ? undefined
    : navigator,
): "macOS" | "Windows" | null {
  if (!browser) return null;

  const { userAgentData, platform, userAgent, maxTouchPoints } = browser;
  // iPadOS can identify itself as a Mac when requesting desktop sites.
  if (
    userAgentData?.mobile ||
    /Android|iPhone|iPad|iPod|Mobile|Windows Phone/i.test(userAgent) ||
    (/Mac/i.test(platform) && maxTouchPoints > 1)
  ) {
    return null;
  }

  const system = userAgentData?.platform || platform || userAgent;
  if (/Windows|Win32|Win64|WinNT/i.test(system)) return "Windows";
  if (/macOS|MacIntel|MacPPC|Macintosh|Mac OS/i.test(system)) return "macOS";
  return null;
}
