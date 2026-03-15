const script = document.createElement("script");
script.src = "https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js";
script.onload = () => {
  mermaid.initialize({
    startOnLoad: false,
    theme: "dark",
    flowchart: { curve: "monotoneY", padding: 16 },
    themeVariables: {
      primaryColor: "#1c1c2b",
      primaryTextColor: "#ededf5",
      primaryBorderColor: "rgba(107,138,255,0.3)",
      lineColor: "rgba(107,138,255,0.4)",
      secondaryColor: "#1c1c2b",
      tertiaryColor: "#1c1c2b",
      fontFamily: "system-ui, -apple-system, sans-serif",
      fontSize: "14px",
      nodeBorder: "rgba(107,138,255,0.3)",
      mainBkg: "#1c1c2b",
      edgeLabelBackground: "#13131e",
      clusterBkg: "transparent",
      clusterBorder: "transparent",
    },
  });
  mermaid.run({ querySelector: ".mermaid" });
};
document.head.appendChild(script);
