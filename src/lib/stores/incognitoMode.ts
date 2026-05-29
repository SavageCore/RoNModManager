import { writable } from "svelte/store";
import type { InstalledModGroup } from "$lib/types";

export const incognitoMode = writable<boolean>(false);

export const DUMMY_MOD_GROUPS: InstalledModGroup[] = [
  {
    name: "CQB_Tactics_Pack",
    displayName: "CQB Tactics Pack",
    sourceUrl: "https://www.nexusmods.com/readyornot/mods/1042",
    managedByManifest: true,
    installedAt: 1736899200,
    files: [
      {
        name: "CQB_Tactics_Pack_P.pak",
        path: "/mods/CQB_Tactics_Pack_P.pak",
        exists: true,
      },
    ],
  },
  {
    name: "Urban_Warfare_Bundle",
    displayName: "Urban Warfare Bundle",
    sourceUrl: "https://www.nexusmods.com/readyornot/mods/2187",
    managedByManifest: true,
    installedAt: 1738540800,
    files: [
      {
        name: "Urban_Warfare_Bundle_P.pak",
        path: "/mods/Urban_Warfare_Bundle_P.pak",
        exists: true,
      },
      {
        name: "Urban_Warfare_Bundle_Audio_P.pak",
        path: "/mods/Urban_Warfare_Bundle_Audio_P.pak",
        exists: true,
      },
    ],
  },
  {
    name: "Realistic_SWAT_Equipment",
    displayName: "Realistic SWAT Equipment",
    sourceUrl: "https://mod.io/g/readyornot/m/realistic-swat-equipment",
    managedByManifest: false,
    installedAt: 1740009600,
    files: [
      {
        name: "Realistic_SWAT_Equipment_P.pak",
        path: "/mods/Realistic_SWAT_Equipment_P.pak",
        exists: true,
      },
    ],
  },
  {
    name: "Breach_And_Clear_Maps",
    displayName: "Breach and Clear Maps",
    sourceUrl: "https://www.nexusmods.com/readyornot/mods/3301",
    managedByManifest: true,
    installedAt: 1741305600,
    files: [
      {
        name: "Breach_And_Clear_Maps_P.pak",
        path: "/mods/Breach_And_Clear_Maps_P.pak",
        exists: true,
      },
      {
        name: "Breach_And_Clear_Maps_Worlds_P.pak",
        path: "/mods/Breach_And_Clear_Maps_Worlds_P.pak",
        exists: true,
      },
      {
        name: "Breach_And_Clear_Maps_Audio_P.pak",
        path: "/mods/Breach_And_Clear_Maps_Audio_P.pak",
        exists: true,
      },
    ],
  },
  {
    name: "NightVision_Overhaul",
    displayName: "Night Vision Overhaul",
    sourceUrl: "https://mod.io/g/readyornot/m/nightvision-overhaul",
    managedByManifest: false,
    installedAt: 1742601600,
    files: [
      {
        name: "NightVision_Overhaul_P.pak",
        path: "/mods/NightVision_Overhaul_P.pak",
        exists: true,
      },
    ],
  },
  {
    name: "Radio_Comms_Rework",
    displayName: "Radio Comms Rework",
    sourceUrl: "https://www.nexusmods.com/readyornot/mods/4455",
    managedByManifest: true,
    installedAt: 1744243200,
    files: [
      {
        name: "Radio_Comms_Rework_P.pak",
        path: "/mods/Radio_Comms_Rework_P.pak",
        exists: true,
      },
    ],
  },
  {
    name: "Suspect_AI_Overhaul",
    displayName: "Suspect AI Overhaul",
    sourceUrl: "https://www.nexusmods.com/readyornot/mods/5123",
    managedByManifest: true,
    installedAt: 1745539200,
    files: [
      {
        name: "Suspect_AI_Overhaul_P.pak",
        path: "/mods/Suspect_AI_Overhaul_P.pak",
        exists: true,
      },
      {
        name: "Suspect_AI_Overhaul_Data_P.pak",
        path: "/mods/Suspect_AI_Overhaul_Data_P.pak",
        exists: true,
      },
    ],
  },
  {
    name: "Modular_Gear_Pack",
    displayName: "Modular Gear Pack",
    sourceUrl: "https://mod.io/g/readyornot/m/modular-gear-pack",
    managedByManifest: false,
    installedAt: 1746662400,
    files: [
      {
        name: "Modular_Gear_Pack_P.pak",
        path: "/mods/Modular_Gear_Pack_P.pak",
        exists: true,
      },
    ],
  },
  {
    name: "Tactical_HUD",
    displayName: "Tactical HUD",
    sourceUrl: "https://www.nexusmods.com/readyornot/mods/6078",
    managedByManifest: false,
    installedAt: 1747612800,
    files: [
      {
        name: "Tactical_HUD_P.pak",
        path: "/mods/Tactical_HUD_P.pak",
        exists: true,
      },
    ],
  },
  {
    name: "Mission_Pack_Vol1",
    displayName: "Mission Pack Vol. 1",
    sourceUrl: "https://www.nexusmods.com/readyornot/mods/7290",
    managedByManifest: true,
    installedAt: 1748304000,
    files: [
      {
        name: "Mission_Pack_Vol1_P.pak",
        path: "/mods/Mission_Pack_Vol1_P.pak",
        exists: true,
      },
      {
        name: "Mission_Pack_Vol1_Worlds_P.pak",
        path: "/mods/Mission_Pack_Vol1_Worlds_P.pak",
        exists: true,
      },
    ],
  },
];

export const DUMMY_PROFILE_MODS: string[] = DUMMY_MOD_GROUPS.map((g) => g.name);

export const DUMMY_COLLECTIONS: Record<string, string[]> = {
  Favourites: ["CQB_Tactics_Pack", "Tactical_HUD", "NightVision_Overhaul"],
  "AI & Gameplay": ["Suspect_AI_Overhaul", "Modular_Gear_Pack"],
};

export const DUMMY_COLLECTION_COLORS: Record<string, string> = {
  Favourites: "#f59e0b",
  "AI & Gameplay": "#8b5cf6",
};

export const DUMMY_TAGS: Record<string, string[]> = {
  audio: ["Radio_Comms_Rework", "Urban_Warfare_Bundle"],
  equipment: ["Realistic_SWAT_Equipment", "Modular_Gear_Pack"],
  map: ["Breach_And_Clear_Maps", "Mission_Pack_Vol1"],
  visual: ["NightVision_Overhaul", "Tactical_HUD"],
};
