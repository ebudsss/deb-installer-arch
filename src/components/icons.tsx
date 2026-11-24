import type { SVGProps } from "react";

/**
 * Ikon garis 24×24 dengan `currentColor`, agar otomatis mengikuti tema.
 * Satu set saja supaya ketebalan garis dan proporsi konsisten.
 */
type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function Icon({ size = 16, children, ...rest }: IconProps & { children: React.ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable="false"
      {...rest}
    >
      {children}
    </svg>
  );
}

export const IconHome = (p: IconProps) => (
  <Icon {...p}>
    <path d="M3.5 10.5 12 3.8l8.5 6.7" />
    <path d="M5.5 9.6V20h13V9.6" />
    <path d="M9.8 20v-5.6h4.4V20" />
  </Icon>
);

export const IconBox = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 3.2 20 7.4v9.2L12 20.8 4 16.6V7.4z" />
    <path d="M4 7.4 12 11.6l8-4.2" />
    <path d="M12 11.6v9.2" />
  </Icon>
);

export const IconSettings = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="2.9" />
    <path d="M19.4 14.2a7.6 7.6 0 0 0 0-4.4l1.6-1.2-1.8-3.1-1.9.7a7.7 7.7 0 0 0-3.8-2.2L13.2 2h-3.6l-.3 2a7.7 7.7 0 0 0-3.8 2.2l-1.9-.7L1.8 8.6l1.6 1.2a7.6 7.6 0 0 0 0 4.4l-1.6 1.2 1.8 3.1 1.9-.7a7.7 7.7 0 0 0 3.8 2.2l.3 2h3.6l.3-2a7.7 7.7 0 0 0 3.8-2.2l1.9.7 1.8-3.1z" />
  </Icon>
);

export const IconUpload = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 15.5V4.2" />
    <path d="M7.6 8.6 12 4.2l4.4 4.4" />
    <path d="M4.2 15.2v3.1a1.9 1.9 0 0 0 1.9 1.9h11.8a1.9 1.9 0 0 0 1.9-1.9v-3.1" />
  </Icon>
);

export const IconPackage = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 2.8 20.2 7v10L12 21.2 3.8 17V7z" />
    <path d="M3.8 7 12 11.2 20.2 7" />
    <path d="M12 11.2v10" />
    <path d="M7.9 4.9 16.1 9.1" />
  </Icon>
);

export const IconSearch = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="10.8" cy="10.8" r="6.3" />
    <path d="m15.6 15.6 4 4" />
  </Icon>
);

export const IconRefresh = (p: IconProps) => (
  <Icon {...p}>
    <path d="M20 12a8 8 0 1 1-2.6-5.9" />
    <path d="M20 4.2V9h-4.8" />
  </Icon>
);

export const IconTrash = (p: IconProps) => (
  <Icon {...p}>
    <path d="M4.5 7h15" />
    <path d="M9.4 7V4.9h5.2V7" />
    <path d="M6.4 7l.8 12.1a1.9 1.9 0 0 0 1.9 1.8h5.8a1.9 1.9 0 0 0 1.9-1.8L17.6 7" />
    <path d="M10.4 11v6M13.6 11v6" />
  </Icon>
);

export const IconCheck = (p: IconProps) => (
  <Icon {...p}>
    <path d="m4.8 12.6 4.6 4.6 9.8-10" />
  </Icon>
);

export const IconAlert = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 4.2 21 19.4H3z" />
    <path d="M12 9.6v4.4" />
    <circle cx="12" cy="16.6" r="0.9" fill="currentColor" stroke="none" />
  </Icon>
);

export const IconInfo = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="8.6" />
    <path d="M12 11v5.2" />
    <circle cx="12" cy="8" r="0.9" fill="currentColor" stroke="none" />
  </Icon>
);

export const IconX = (p: IconProps) => (
  <Icon {...p}>
    <path d="m6 6 12 12M18 6 6 18" />
  </Icon>
);

export const IconFolder = (p: IconProps) => (
  <Icon {...p}>
    <path d="M3.6 7.4A1.9 1.9 0 0 1 5.5 5.5h3.7l2 2.4h7.3a1.9 1.9 0 0 1 1.9 1.9v7.7a1.9 1.9 0 0 1-1.9 1.9H5.5a1.9 1.9 0 0 1-1.9-1.9z" />
  </Icon>
);

export const IconTerminal = (p: IconProps) => (
  <Icon {...p}>
    <rect x="2.8" y="4.4" width="18.4" height="15.2" rx="2" />
    <path d="m7 10 2.6 2.4L7 14.8" />
    <path d="M12.4 15.2h4.2" />
  </Icon>
);

export const IconShield = (p: IconProps) => (
  <Icon {...p}>
    <path d="M12 3.2 19.4 6v5.7c0 4.3-3 7.2-7.4 9-4.4-1.8-7.4-4.7-7.4-9V6z" />
    <path d="m9.2 12 2 2 3.6-3.8" />
  </Icon>
);

export const IconArrowLeft = (p: IconProps) => (
  <Icon {...p}>
    <path d="M19 12H5.4" />
    <path d="m10.6 6.4-5.6 5.6 5.6 5.6" />
  </Icon>
);

export const IconLayers = (p: IconProps) => (
  <Icon {...p}>
    <path d="m12 3.4 8.2 4.2-8.2 4.2L3.8 7.6z" />
    <path d="m3.8 12 8.2 4.2 8.2-4.2" />
    <path d="m3.8 16.4 8.2 4.2 8.2-4.2" />
  </Icon>
);

export const IconClock = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="8.6" />
    <path d="M12 7.6V12l3 1.8" />
  </Icon>
);

export const IconGlobe = (p: IconProps) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="8.6" />
    <path d="M3.6 12h16.8" />
    <path d="M12 3.4c2.2 2.4 3.4 5.4 3.4 8.6S14.2 18.2 12 20.6C9.8 18.2 8.6 15.2 8.6 12S9.8 5.8 12 3.4z" />
  </Icon>
);

export const IconFile = (p: IconProps) => (
  <Icon {...p}>
    <path d="M6.4 3.6h7.2l4 4v12.8H6.4z" />
    <path d="M13.6 3.6v4h4" />
  </Icon>
);

