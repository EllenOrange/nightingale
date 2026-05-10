import type { MicReactiveRef } from "@/hooks/use-mic-reactive";
import { useReactiveShaderUniforms } from "@/hooks/use-reactive-shader-uniforms";
import { Canvas } from "@react-three/fiber";
import { shaders, vertexShader } from "./shaders";

interface ShaderVisualizerProps {
  shaderIndex: number;
  isPlaying: boolean;
  customFragment?: string;
  reactiveRef?: MicReactiveRef;
}

const ShaderQuad = ({
  shaderIndex,
  isPlaying,
  customFragment,
  reactiveRef,
}: ShaderVisualizerProps) => {
  const fragmentShader = customFragment ?? shaders[shaderIndex].fragmentShader;
  const uniforms = useReactiveShaderUniforms(reactiveRef, isPlaying);

  return (
    <mesh>
      <planeGeometry args={[2, 2]} />
      <shaderMaterial
        key={customFragment ? "custom" : shaderIndex}
        vertexShader={vertexShader}
        fragmentShader={fragmentShader}
        uniforms={uniforms}
      />
    </mesh>
  );
};

export const ShaderVisualizer = (props: ShaderVisualizerProps) => (
  <div className="fixed inset-0">
    <Canvas flat dpr={1}>
      <ShaderQuad {...props} />
    </Canvas>
  </div>
);
