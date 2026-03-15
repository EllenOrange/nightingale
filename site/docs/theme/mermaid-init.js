const script = document.createElement("script");
script.src = "https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js";
script.onload = () => {
  mermaid.initialize({
    startOnLoad: false,
    theme: "dark",
    themeVariables: {
      primaryColor: "#1c1c2b",
      primaryTextColor: "#ededf5",
      primaryBorderColor: "rgba(255,255,255,0.12)",
      lineColor: "#6b8aff",
      secondaryColor: "#1c1c2b",
      tertiaryColor: "#1c1c2b",
      fontFamily: "system-ui, -apple-system, sans-serif",
    },
  });
  mermaid.run({ querySelector: ".mermaid" });
};
document.head.appendChild(script);
