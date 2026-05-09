import { Canvas, useFrame } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import type { MicReactiveRef } from "@/hooks/use-mic-reactive";
import { shaders, vertexShader } from "./shaders";

const WAVE_BINS = 256;

/**
 * Mic-toggle ease-in/out time constant (seconds). The Rust analyzer already
 * delivers shader-ready values; this is the only audio-touching state left
 * on the frontend, and it exists purely to fade reactive influence in/out
 * smoothly when the user toggles the mic.
 */
const TAU_MIC_BLEND_SEC = 0.4;

const NEUTRAL_PITCH = 0.5;
const NEUTRAL_CENTROID = 0.5;

/**
 * Time-rate modulation by audio. `uTimeFast` speeds up while singing,
 * `uTimeSlow` eases down. Rates are blended from the smoothed Rust volume
 * times the mic-blend factor, so a silent mic looks identical to no mic.
 */
const TIME_FAST_GAIN = 1.4;
const TIME_SLOW_GAIN = 0.65;
const TIME_SLOW_FLOOR = 0.35;

interface Props {
  shaderIndex: number;
  isPlaying: boolean;
  customFragment?: string;
  reactiveRef?: MicReactiveRef;
}

interface AudioUniforms {
  uAudioReactive: { value: number };
  uVolume: { value: number };
  uLow: { value: number };
  uMid: { value: number };
  uHigh: { value: number };
  uCentroid: { value: number };
  uPitch: { value: number };
  uEnergy: { value: number };
  uHue: { value: number };
  uFlow: { value: number };
  uTimeFast: { value: number };
  uTimeSlow: { value: number };
  uWave: { value: THREE.DataTexture };
}

function ema(prev: number, next: number, delta: number, tau: number): number {
  const a = 1 - Math.exp(-delta / Math.max(1e-6, tau));
  return prev + (next - prev) * a;
}

const ShaderQuad = ({ shaderIndex, isPlaying, customFragment, reactiveRef }: Props) => {
  const materialRef = useRef<THREE.ShaderMaterial>(null!);
  const timeRef = useRef(0);
  const timeFastRef = useRef(0);
  const timeSlowRef = useRef(0);
  const reactiveBlendRef = useRef(0);

  const waveBufRef = useRef<Uint8Array>(new Uint8Array(WAVE_BINS).fill(127));
  const waveTexRef = useRef<THREE.DataTexture | null>(null);
  if (waveTexRef.current === null) {
    const tex = new THREE.DataTexture(
      waveBufRef.current,
      WAVE_BINS,
      1,
      THREE.RedFormat,
      THREE.UnsignedByteType,
    );
    tex.magFilter = THREE.LinearFilter;
    tex.minFilter = THREE.LinearFilter;
    tex.wrapS = THREE.ClampToEdgeWrapping;
    tex.wrapT = THREE.ClampToEdgeWrapping;
    tex.needsUpdate = true;
    waveTexRef.current = tex;
  }

  const fragment = customFragment ?? shaders[shaderIndex].fragmentShader;

  const uniforms = useMemo(() => {
    const audio: AudioUniforms = {
      uAudioReactive: { value: 0 },
      uVolume: { value: 0 },
      uLow: { value: 0 },
      uMid: { value: 0 },
      uHigh: { value: 0 },
      uCentroid: { value: NEUTRAL_CENTROID },
      uPitch: { value: NEUTRAL_PITCH },
      uEnergy: { value: 0 },
      uHue: { value: 0 },
      uFlow: { value: 0 },
      uTimeFast: { value: 0 },
      uTimeSlow: { value: 0 },
      uWave: { value: waveTexRef.current as THREE.DataTexture },
    };
    return {
      uTime: { value: 0 },
      ...audio,
    };
  }, [shaderIndex, customFragment]);

  useFrame((_, delta) => {
    const reactive = reactiveRef?.current ?? null;

    const targetBlend = reactive != null ? 1 : 0;
    reactiveBlendRef.current = ema(reactiveBlendRef.current, targetBlend, delta, TAU_MIC_BLEND_SEC);

    const audioVol = (reactive?.volume ?? 0) * reactiveBlendRef.current;
    const fastRate = 1 + audioVol * TIME_FAST_GAIN;
    const slowRate = Math.max(TIME_SLOW_FLOOR, 1 - audioVol * TIME_SLOW_GAIN);

    if (isPlaying) {
      timeRef.current += delta;
      timeFastRef.current += delta * fastRate;
      timeSlowRef.current += delta * slowRate;
    }

    const u = materialRef.current.uniforms as unknown as AudioUniforms & {
      uTime: { value: number };
    };
    u.uTime.value = timeRef.current;
    u.uTimeFast.value = timeFastRef.current;
    u.uTimeSlow.value = timeSlowRef.current;
    u.uAudioReactive.value = reactiveBlendRef.current;
    u.uVolume.value = reactive?.volume ?? 0;
    u.uLow.value = reactive?.low ?? 0;
    u.uMid.value = reactive?.mid ?? 0;
    u.uHigh.value = reactive?.high ?? 0;
    u.uCentroid.value = reactive?.centroid ?? NEUTRAL_CENTROID;
    u.uPitch.value = reactive?.pitch ?? NEUTRAL_PITCH;
    u.uEnergy.value = reactive?.energy ?? 0;
    u.uHue.value = reactive?.hue ?? 0;
    u.uFlow.value = reactive?.flow ?? 0;

    const wave = reactive?.wave;
    const buf = waveBufRef.current;
    if (wave && wave.length === WAVE_BINS) {
      for (let i = 0; i < WAVE_BINS; i++) {
        const v = wave[i] * 127.5 + 127.5;
        buf[i] = v < 0 ? 0 : v > 255 ? 255 : v | 0;
      }
      const tex = waveTexRef.current;
      if (tex) tex.needsUpdate = true;
    } else {
      let stale = false;
      for (let i = 0; i < WAVE_BINS; i++) {
        if (buf[i] !== 127) {
          buf[i] = 127;
          stale = true;
        }
      }
      if (stale && waveTexRef.current) waveTexRef.current.needsUpdate = true;
    }
  });

  return (
    <mesh>
      <planeGeometry args={[2, 2]} />
      <shaderMaterial
        ref={materialRef}
        key={customFragment ? "custom" : shaderIndex}
        vertexShader={vertexShader}
        fragmentShader={fragment}
        uniforms={uniforms}
      />
    </mesh>
  );
};

export const ShaderVisualizer = ({
  shaderIndex,
  isPlaying,
  customFragment,
  reactiveRef,
}: Props) => {
  return (
    <div className="fixed inset-0">
      <Canvas flat dpr={1}>
        <ShaderQuad
          shaderIndex={shaderIndex}
          isPlaying={isPlaying}
          customFragment={customFragment}
          reactiveRef={reactiveRef}
        />
      </Canvas>
    </div>
  );
};
