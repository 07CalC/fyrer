/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: ['selector', '[data-theme="dark"]'],
  content: ['./src/**/*.{js,jsx,ts,tsx,mdx}', './docs/**/*.{md,mdx}'],
  theme: {
    extend: {
      colors: {
        base: '#08090a',
        surface: '#0f1113',
        border: '#1c1f22',
        muted: '#8a8f98',
        offwhite: '#eceef0',
        accent: '#ff4d00',
        accent2: '#ff6b2b',
      },
      fontFamily: {
        sans: ['Geist', 'Inter', 'system-ui', 'sans-serif'],
        mono: ['Geist Mono', 'JetBrains Mono', 'ui-monospace', 'SFMono-Regular', 'monospace'],
      },
      borderRadius: {
        container: '14px',
        card: '12px',
      },
      animation: {
        ping: 'ping 1.4s cubic-bezier(0,0,0.2,1) infinite',
      },
    },
  },
  plugins: [],
  corePlugins: {preflight: false},
};
