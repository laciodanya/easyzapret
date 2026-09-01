/** Parse Happ-style server remarks into a country code + clean title. */

const NAME_TO_CC: [string, string][] = [
  ["germany", "de"],
  ["deutschland", "de"],
  ["германия", "de"],
  ["netherlands", "nl"],
  ["holland", "nl"],
  ["нидерланды", "nl"],
  ["голландия", "nl"],
  ["finland", "fi"],
  ["финляндия", "fi"],
  ["france", "fr"],
  ["франция", "fr"],
  ["sweden", "se"],
  ["швеция", "se"],
  ["norway", "no"],
  ["норвегия", "no"],
  ["denmark", "dk"],
  ["дания", "dk"],
  ["austria", "at"],
  ["австрия", "at"],
  ["switzerland", "ch"],
  ["швейцария", "ch"],
  ["poland", "pl"],
  ["польша", "pl"],
  ["turkey", "tr"],
  ["türkiye", "tr"],
  ["турция", "tr"],
  ["ukraine", "ua"],
  ["украина", "ua"],
  ["russia", "ru"],
  ["россия", "ru"],
  ["united kingdom", "gb"],
  ["great britain", "gb"],
  ["england", "gb"],
  ["великобритания", "gb"],
  ["united states", "us"],
  ["usa", "us"],
  ["сша", "us"],
  ["canada", "ca"],
  ["канада", "ca"],
  ["japan", "jp"],
  ["япония", "jp"],
  ["singapore", "sg"],
  ["сингапур", "sg"],
  ["hong kong", "hk"],
  ["taiwan", "tw"],
  ["korea", "kr"],
  ["italy", "it"],
  ["италия", "it"],
  ["spain", "es"],
  ["испания", "es"],
  ["portugal", "pt"],
  ["czech", "cz"],
  ["чехия", "cz"],
  ["latvia", "lv"],
  ["латвия", "lv"],
  ["lithuania", "lt"],
  ["литва", "lt"],
  ["estonia", "ee"],
  ["эстония", "ee"],
  ["romania", "ro"],
  ["bulgaria", "bg"],
  ["kazakhstan", "kz"],
  ["казахстан", "kz"],
  ["uae", "ae"],
  ["emirates", "ae"],
  ["india", "in"],
  ["brazil", "br"],
  ["australia", "au"],
  ["moldova", "md"],
  ["georgia", "ge"],
  ["armenia", "am"],
  ["israel", "il"],
  ["cyprus", "cy"],
];

const ISO = new Set([
  "ad","ae","af","al","am","ar","at","au","az","ba","be","bg","bh","br","by","ca","ch","cl","cn","cy","cz","de","dk","ee","eg","es","fi","fr","gb","ge","gr","hk","hr","hu","id","ie","il","in","iq","ir","is","it","jp","kg","kr","kz","lt","lu","lv","md","me","mk","mx","my","nl","no","nz","pl","pt","qa","ro","rs","ru","sa","se","sg","si","sk","th","tr","tw","ua","us","uz","vn",
]);

/** Restore UTF-8 that was decoded as Latin-1 (`ðŸ‡©` → 🇩🇪). */
export function repairMojibake(s: string): string {
  if (![...s].some((c) => {
    const code = c.charCodeAt(0);
    return code >= 0x80 && code <= 0xff;
  })) {
    return s;
  }
  const bytes = Uint8Array.from([...s].map((c) => c.charCodeAt(0) & 0xff));
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return s;
  }
}

export function countryFromName(name: string): string | null {
  const text = repairMojibake(name);
  const flags = text.match(/\p{Regional_Indicator}{2}/gu);
  if (flags && flags[0]) {
    const cc = regionalToIso(flags[0]);
    if (cc) return cc;
  }
  const m = text.match(/(?:^|[\s\[(])([A-Z]{2})(?:$|[\s\])\-_|])/);
  if (m && ISO.has(m[1].toLowerCase())) return m[1].toLowerCase();
  const lower = text.toLowerCase();
  for (const [needle, cc] of NAME_TO_CC) {
    if (lower.includes(needle)) return cc;
  }
  return null;
}

export function countryFromAddress(address: string): string | null {
  const host = address.split(":")[0].toLowerCase();
  const first = host.split(".")[0] ?? "";
  const m = first.match(/^([a-z]{2})(?:[-_]|\d|$)/);
  if (m && ISO.has(m[1])) return m[1];
  return null;
}

function regionalToIso(pair: string): string | null {
  const chars = [...pair];
  if (chars.length !== 2) return null;
  const a = chars[0].codePointAt(0);
  const b = chars[1].codePointAt(0);
  if (!a || !b) return null;
  const cc = String.fromCharCode(a - 0x1f1e6 + 65, b - 0x1f1e6 + 65).toLowerCase();
  return /^[a-z]{2}$/.test(cc) ? cc : null;
}

export function displayServerName(name: string): string {
  return repairMojibake(name)
    .replace(/\p{Regional_Indicator}{2}/gu, "")
    .replace(/\s{2,}/g, " ")
    .trim();
}
