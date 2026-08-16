import { useStudioSession } from "./useStudioSession";
import { Workbench } from "./Workbench";

export function App() {
  const session = useStudioSession();
  return <Workbench session={session} />;
}
