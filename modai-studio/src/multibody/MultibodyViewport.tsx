import { Pause, Play } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { DiagramDocument, SimulationResult } from "../api/types";
import { IconButton } from "../shell/IconButton";
import { buildMultibodyScene, type BodyPose, type MultibodyScene } from "./poseExtract";

function makeMesh(body: BodyPose): THREE.Mesh {
  let geom: THREE.BufferGeometry;
  if (body.shape === "cylinder") {
    geom = new THREE.CylinderGeometry(body.size[0], body.size[0], body.size[1], 16);
  } else if (body.shape === "sphere") {
    geom = new THREE.SphereGeometry(body.size[0], 16, 12);
  } else {
    geom = new THREE.BoxGeometry(body.size[0], body.size[1], body.size[2]);
  }
  const mat = new THREE.MeshStandardMaterial({
    color: body.color,
    metalness: 0.15,
    roughness: 0.55,
  });
  const mesh = new THREE.Mesh(geom, mat);
  mesh.castShadow = true;
  mesh.receiveShadow = true;
  mesh.name = body.id;
  return mesh;
}

function applyPose(mesh: THREE.Mesh, body: BodyPose, frame: number) {
  const i = Math.max(0, Math.min(frame, body.positions.length - 1));
  const p = body.positions[i] ?? [0, 0, 0];
  // Modelica MultiBody: y up in many models; three.js uses y-up — map (x,y,z) -> (x,z,y) for ground plane xz.
  mesh.position.set(p[0], p[2], -p[1]);
  if (body.eulers?.[i]) {
    const e = body.eulers[i]!;
    mesh.rotation.set(e[0], e[2], -e[1], "XYZ");
  }
}

export function MultibodyViewport({
  diagram,
  result,
}: {
  diagram: DiagramDocument | null;
  result: SimulationResult | null;
}) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const sceneRef = useRef<MultibodyScene | null>(null);
  const meshesRef = useRef<Map<string, THREE.Mesh>>(new Map());
  const frameRef = useRef(0);
  const playingRef = useRef(false);
  const rafRef = useRef(0);

  const sceneData = useMemo(() => buildMultibodyScene(diagram, result), [diagram, result]);
  const [frame, setFrame] = useState(0);
  const [playing, setPlaying] = useState(false);
  const maxFrame = Math.max(0, sceneData.time.length - 1);

  useEffect(() => {
    sceneRef.current = sceneData;
    setFrame(0);
    frameRef.current = 0;
    setPlaying(false);
    playingRef.current = false;
  }, [sceneData]);

  useEffect(() => {
    frameRef.current = frame;
    for (const body of sceneRef.current?.bodies ?? []) {
      const mesh = meshesRef.current.get(body.id);
      if (mesh) applyPose(mesh, body, frame);
    }
  }, [frame]);

  useEffect(() => {
    playingRef.current = playing;
  }, [playing]);

  useEffect(() => {
    const el = mountRef.current;
    if (!el) return;

    const width = el.clientWidth || 480;
    const height = el.clientHeight || 220;
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(width, height);
    renderer.shadowMap.enabled = true;
    el.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x15191e);

    const camera = new THREE.PerspectiveCamera(45, width / height, 0.05, 200);
    camera.position.set(2.2, 1.6, 2.8);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.target.set(0, 0.2, 0);

    const hemi = new THREE.HemisphereLight(0xddeeff, 0x223344, 0.9);
    scene.add(hemi);
    const dir = new THREE.DirectionalLight(0xffffff, 0.85);
    dir.position.set(3, 6, 2);
    dir.castShadow = true;
    scene.add(dir);

    const grid = new THREE.GridHelper(6, 12, 0x3a424d, 0x2a3038);
    scene.add(grid);
    const axes = new THREE.AxesHelper(0.6);
    scene.add(axes);

    const group = new THREE.Group();
    scene.add(group);

    const rebuild = (data: MultibodyScene) => {
      while (group.children.length) {
        const c = group.children.pop()!;
        group.remove(c);
        if (c instanceof THREE.Mesh) {
          c.geometry.dispose();
          (c.material as THREE.Material).dispose();
        }
      }
      meshesRef.current.clear();
      for (const body of data.bodies) {
        const mesh = makeMesh(body);
        applyPose(mesh, body, frameRef.current);
        group.add(mesh);
        meshesRef.current.set(body.id, mesh);
      }
    };
    rebuild(sceneData);

    let lastTs = 0;
    const tick = (ts: number) => {
      rafRef.current = requestAnimationFrame(tick);
      if (playingRef.current && sceneRef.current && sceneRef.current.time.length > 1) {
        if (ts - lastTs > 33) {
          lastTs = ts;
          const next = frameRef.current + 1;
          const capped = next > sceneRef.current.time.length - 1 ? 0 : next;
          frameRef.current = capped;
          setFrame(capped);
        }
      }
      controls.update();
      renderer.render(scene, camera);
    };
    rafRef.current = requestAnimationFrame(tick);

    const onResize = () => {
      const w = el.clientWidth || 480;
      const h = el.clientHeight || 220;
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h);
    };
    const ro = new ResizeObserver(onResize);
    ro.observe(el);

    // Rebuild when scene identity changes via closure refresh on dep — handled below.
    (el as HTMLDivElement & { __rebuild?: (d: MultibodyScene) => void }).__rebuild = rebuild;

    return () => {
      cancelAnimationFrame(rafRef.current);
      ro.disconnect();
      controls.dispose();
      renderer.dispose();
      el.removeChild(renderer.domElement);
      meshesRef.current.clear();
    };
  }, []);

  useEffect(() => {
    const el = mountRef.current as
      | (HTMLDivElement & { __rebuild?: (d: MultibodyScene) => void })
      | null;
    el?.__rebuild?.(sceneData);
  }, [sceneData]);

  const tNow = sceneData.time[frame] ?? 0;
  const sourceLabel =
    sceneData.source === "simulation"
      ? "from simulation"
      : sceneData.source === "diagram"
        ? "diagram preview"
        : "no multibody";

  return (
    <div className="mb3d-wrap">
      <div className="mb3d-toolbar">
        <IconButton
          icon={playing ? Pause : Play}
          labelKey={playing ? "pause" : "run"}
          disabled={maxFrame < 1}
          onClick={() => setPlaying((p) => !p)}
        />
        <label className="mb3d-scrub">
          t={tNow.toFixed(3)}
          <input
            type="range"
            min={0}
            max={maxFrame}
            value={frame}
            disabled={maxFrame < 1}
            onChange={(ev) => {
              setPlaying(false);
              setFrame(Number(ev.target.value));
            }}
          />
        </label>
        <span className="status-pill">{sourceLabel}</span>
        <span className="status-pill">{sceneData.bodies.length} bodies</span>
      </div>
      <div ref={mountRef} className="mb3d-canvas" />
      {!sceneData.bodies.length ? (
        <div className="mb3d-empty">
          Drop MultiBody World / Body / FixedTranslation from Library, or Run a model that outputs r_0.
        </div>
      ) : null}
    </div>
  );
}
