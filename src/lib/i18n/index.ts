// Translations by area of the app, merged. Each file exports `fr` and `es`, keyed by the
// English text exactly as it appears in the code.
import * as archive from "./archive";
import * as core from "./core";
import * as licence from "./licence";
import * as panels from "./panels";
import * as service from "./service";
import * as style from "./style";

const areas = [core, licence, service, panels, style, archive];

export const fr: Record<string, string> = Object.assign({}, ...areas.map((a) => a.fr));
export const es: Record<string, string> = Object.assign({}, ...areas.map((a) => a.es));
