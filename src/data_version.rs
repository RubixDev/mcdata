//! Defines data version constants for various Minecraft versions.

macro_rules! data_versions {
    ($( $name:ident: $mc:literal => $version:literal ),* $(,)?) => {
        $(
            #[doc = concat!("Data version ", stringify!($version), " for Minecraft ", $mc, ".")]
            pub const $name: i32 = $version;
        )*
    };
}

// JS one-liner to run on https://minecraft.wiki/w/Data_version in dev-tools console:
// ```js
// console.log([...document.getElementsByTagName('tbody')[0].children].map(child => [child.firstElementChild.firstElementChild.innerText.replace(/^Java Edition /, ''), child.lastElementChild.innerText]).filter(([mc, data]) => /^\d+(\.\d+)+$/.test(mc) && data !== 'N/A').map(([mc, data]) => `    MC_${mc.replaceAll('.', '_')}: "${mc}" => ${data},\n`).join(''))
// ```
data_versions! {
    MC_1_14: "1.14" => 1952,
    MC_1_14_1: "1.14.1" => 1957,
    MC_1_14_2: "1.14.2" => 1963,
    MC_1_14_3: "1.14.3" => 1968,
    MC_1_14_4: "1.14.4" => 1976,
    MC_1_15: "1.15" => 2225,
    MC_1_15_1: "1.15.1" => 2227,
    MC_1_15_2: "1.15.2" => 2230,
    MC_1_16: "1.16" => 2566,
    MC_1_16_1: "1.16.1" => 2567,
    MC_1_16_2: "1.16.2" => 2578,
    MC_1_16_3: "1.16.3" => 2580,
    MC_1_16_4: "1.16.4" => 2584,
    MC_1_16_5: "1.16.5" => 2586,
    MC_1_17: "1.17" => 2724,
    MC_1_17_1: "1.17.1" => 2730,
    MC_1_18: "1.18" => 2860,
    MC_1_18_1: "1.18.1" => 2865,
    MC_1_18_2: "1.18.2" => 2975,
    MC_1_19: "1.19" => 3105,
    MC_1_19_1: "1.19.1" => 3117,
    MC_1_19_2: "1.19.2" => 3120,
    MC_1_19_3: "1.19.3" => 3218,
    MC_1_19_4: "1.19.4" => 3337,
    MC_1_20: "1.20" => 3463,
    MC_1_20_1: "1.20.1" => 3465,
    MC_1_20_2: "1.20.2" => 3578,
    MC_1_20_3: "1.20.3" => 3698,
    MC_1_20_4: "1.20.4" => 3700,
    MC_1_20_5: "1.20.5" => 3837,
    MC_1_20_6: "1.20.6" => 3839,
}
