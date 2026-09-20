// One-time Chart.js component registration (tree-shakeable build - Chart.js
// only bundles what's registered). Every chart module imports this file for
// its side effect rather than repeating the registration list.
import {
  BarController,
  BarElement,
  CategoryScale,
  Chart,
  Legend,
  LinearScale,
  LineController,
  LineElement,
  PointElement,
  Tooltip,
} from "chart.js";

Chart.register(
  BarController,
  BarElement,
  CategoryScale,
  LinearScale,
  LineController,
  LineElement,
  PointElement,
  Legend,
  Tooltip,
);

export { Chart };
