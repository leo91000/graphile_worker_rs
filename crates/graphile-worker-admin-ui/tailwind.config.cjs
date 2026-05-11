const { iconsPlugin, getIconCollections } = require("@egoist/tailwindcss-icons");

module.exports = {
  darkMode: "class",
  content: ["./src/**/*.rs", "../graphile-worker-admin-ui-client/src/**/*.rs"],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "Inter",
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "sans-serif",
        ],
      },
    },
  },
  plugins: [
    iconsPlugin({
      collections: getIconCollections(["lucide", "tabler"]),
      extraProperties: {
        display: "inline-block",
        "vertical-align": "-0.16em",
      },
    }),
  ],
};
