Yes. I’d treat the references as **design-system research**, not as sources for individual character designs. The strongest lesson from games like *Brawl Stars*, *Overwatch*, and *Smash Legends* is that a roster works when players can infer a character’s combat behavior from silhouette, proportions, animation, and VFX before learning the kit. *Brawl Stars* explicitly separates Damage, Tank, Support, Controller and other combat classes; *Overwatch* similarly uses role-specific behaviors; and the *Smash Legends* developers specifically describe distinctive appearance and clear attack/hitbox readability as priorities when building characters. ([Supercell Support Portal][1])

For **PewPew Blitz**, there is one particularly important constraint: **weapons are interchangeable**. So I would *not* make “the rocket guy,” “the bow girl,” “the sniper robot,” etc. The character itself should communicate personality, body class and broad combat role, while the equipped weapon remains a separate layer.

## Visual language for the roster

I’d establish these rules before designing individual heroes.

| Attribute         | Visual language                                                                                              |
| ----------------- | ------------------------------------------------------------------------------------------------------------ |
| **Light**         | narrow torso, longer limbs/ears/tail, small feet, tilted poses, asymmetry, lots of negative space            |
| **Normal**        | compact heroic proportions similar to your current model, balanced upper/lower body                          |
| **Heavy**         | wide torso, short legs, oversized hands/feet, low center of gravity                                          |
| **Damage Dealer** | forward-leaning pose, pointed/diagonal shapes, confident expressions                                         |
| **Tank**          | broad shoulders, squares/arcs, planted stance, visually protected core                                       |
| **Helper**        | round/open shapes, backpacks/pouches/companions, welcoming gestures                                          |
| **Controller**    | unusual asymmetry, orbiting/floating pieces, antennae/tendrils/capes, visual suggestion of spatial awareness |

That lets a player immediately read something like **“fast controller”** or **“heavy helper”**, even if both happen to equip the same laser rifle.

I’d keep the actual rendering close to the player model you provided: chunky low-poly shapes, simple faces, very limited texture detail, approximately 3–4 strong colors per character, expressive idle animation, and big readable features visible from a top-down camera.

## Initial character roster ideas

These are deliberately varied so we can discover what belongs in the PewPew Blitz universe rather than committing too early to “humans with costumes.”

| Character  | Role / Body         | Concept                                       | Visual hook                                                                                            |
| ---------- | ------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| **Riff**   | Damage / Light      | Hyperactive jackrabbit courier                | Enormous swept-back ears, cropped jacket, tiny sneakers, permanently leaning forward                   |
| **Torque** | Damage / Heavy      | Cheerful alien beetle                         | Huge rounded shell shoulders, tiny head, four chunky forearms but only two used for weapons            |
| **Mica**   | Damage / Normal     | Gecko stunt pilot                             | Wide sticky fingertips, bright flight scarf, big expressive eyes, long curled tail                     |
| **Flint**  | Damage / Normal     | Living ember creature wearing adventurer gear | Black charcoal body with warm glowing cracks, tiny backpack and oversized eyebrows                     |
| **Fizz**   | Damage / Light      | Carbonated slime alien                        | Bottle-shaped silhouette, translucent-ish jelly crest, constantly bubbling idle motion                 |
| **Cobble** | Tank / Heavy        | Ancient little stone guardian                 | Giant block hands, square torso, tiny legs, moss tuft functioning almost like hair                     |
| **Bumper** | Tank / Normal       | Anthropomorphic mountain ram                  | Huge curled horns create the silhouette; padded sports gear rather than medieval armor                 |
| **Plunk**  | Tank / Heavy        | Squishy gravity-resistant space blob          | Pear-shaped body, tiny feet, enormous belly that compresses and rebounds during movement               |
| **Kettle** | Tank / Heavy        | Friendly ceramic golem                        | Rounded pottery body, visible repaired seams, lid-like helmet, steam puffs when excited                |
| **Rook**   | Tank / Light        | Tall defensive bird construct                 | Very long legs, shield-shaped feather plates; looks fragile but uses clever defensive tech             |
| **Pollen** | Helper / Light      | Tiny moth-like forest traveler                | Giant fluffy antennae, oversized sleeves, satchel full of glowing spores                               |
| **Patch**  | Helper / Normal     | Raccoon workshop tinkerer                     | Rolled sleeves, utility belt, mismatched gloves, permanently carrying too many gadgets                 |
| **Mallow** | Helper / Heavy      | Huge cloud-sheep creature                     | Soft rectangular wool mass, tiny legs and face, little bells and utility packs embedded in wool        |
| **Sprig**  | Helper / Normal     | Walking plant creature                        | Leafy hair, wooden limbs, flower buds changing expression with mood                                    |
| **Orbit**  | Helper / Light      | Small alien with two floating robotic hands   | Tiny central body; detached hands follow it around and mimic gestures during idle animations           |
| **Sumi**   | Controller / Normal | Ink-squid street artist                       | Tentacle “hair,” asymmetrical jacket, animated ink ribbons orbiting slowly around the body             |
| **Prism**  | Controller / Light  | Curious crystalline alien                     | Diamond-shaped head, floating crystal fragments, no visible mouth, expressive glowing eyes             |
| **Tangle** | Controller / Heavy  | Friendly walking root-ball                    | Wide tangled body, little sprouts above the head, roots briefly spread across the ground when stopping |
| **Tock**   | Controller / Normal | Clockwork owl scholar                         | Circular eyes, one slightly larger than the other, rotating feather segments and tiny pendulum         |
| **Hush**   | Controller / Light  | Mischievous cloth spirit                      | Mostly an oversized hood and long sleeves with little feet underneath; face is two simple lights       |

### A few I think are particularly strong

**Cobble** could become one of the visual anchors of the game. He is extremely easy to recognize from above and establishes immediately that PewPew Blitz isn't restricted to human heroes. I’d make him almost comically rectangular, with a tiny happy face set into an enormous stone head.

**Pollen** gives you the opposite silhouette: very small body, enormous antennae, wide sleeves, rapid fluttering idle movements. She could equip a sniper rifle or rocket launcher and still unmistakably look like Pollen.

**Sumi** is a good Controller because the character can communicate “space manipulation” without depending on the equipped weapon. Her tentacles and ink ribbons can spread slightly when using controller abilities and collapse tightly around her while running.

**Plunk** gives the roster some comedy. Rather than the usual muscular tank, he’s essentially a giant happy alien bean. Hits can squash him briefly before he springs back into shape.

**Orbit** is particularly useful for your modular weapon system. One floating hand can hold the equipped weapon while the other participates in emotes, reloads and abilities. Even something oversized like a rocket launcher could look plausible.

**Riff** gives you a very clean Light silhouette. The ears can trail behind while sprinting, stand upright when idle, flatten when hit, and become a major source of personality without complex facial animation.

## Some stranger directions worth exploring

I would not make the entire roster animals and humanoids. A few genuinely odd heroes will make the universe feel much more proprietary.

**Doorstop** — Heavy Tank. A small sentient fortress door that walks on two feet. Huge rectangular silhouette, tiny face in a viewing slot.

**Gloop & Pip** — Normal Helper. A large jelly creature with a tiny creature permanently sitting inside it. The tiny one reacts to combat while Gloop handles movement.

**Skip** — Light Controller. A flat paper-like creature that appears almost two-dimensional and turns sideways when sprinting.

**Lunchbox** — Heavy Helper. A cheerful walking food-storage robot whose torso opens during animations to reveal absurdly organized equipment.

**Noodle** — Light Damage. Extremely long-bodied alien compressed into the game's normal character height; arms and neck stretch slightly during animations.

**Pebble** — Light Tank. A tiny stone creature carrying a huge floating ring around itself. This is useful because it deliberately breaks the assumption that Tank must always equal Heavy.

That last point matters. Your three body categories can create much more interesting combinations if they're **modifiers rather than class restrictions**. A Heavy Damage Dealer and Light Tank will immediately feel different from the expected archetypes.

## Weapon modularity

I would design a universal grip standard into every hero from the beginning:

* approximately the same weapon attachment position;
* standardized one-hand/two-hand animation sockets;
* no permanent costume element occupying the weapon silhouette;
* enough clearance around head/shoulders for large launchers;
* bow/wand-specific hand poses handled by animation rather than character geometry;
* oversized characters scale the weapon *slightly*, not proportionally.

Then use animation to personalize the exact same gun. **Cobble** might hold it rigidly with both hands; **Riff** casually one-hands it; **Orbit** lets a floating hand carry it; **Sumi** wraps a tentacle around the supporting grip. That produces much more personality than giving every character an exclusive weapon.

## The first 8 I would actually prototype

For a first visual exploration sheet, I'd use **Riff, Torque, Cobble, Plunk, Pollen, Orbit, Sumi and Prism**.

That gives us a rabbit, beetle alien, stone creature, blob, moth, unusual alien/robot hybrid, squid creature and crystal being—with Light, Normal and Heavy silhouettes represented and all four roles covered. If those eight can stand next to your existing player model and still look like one coherent game, we'll have found the visual grammar for the rest of the roster.

Also, *Smash Legends* describes a useful production philosophy: start from the gameplay need, develop personality/background around it, then make the model's distinctive visual characteristics strong enough that a diverse cast still feels cohesive. That is exactly the approach I'd use here rather than designing isolated cool-looking mascots. ([smashlegends.com][2])

The next useful step would be a **single lineup concept sheet of 8 characters at equal scale**, using your existing low-poly model as the style anchor. That will tell us much more about silhouette and roster cohesion than polishing one character at a time.

[1]: https://support.supercell.com/brawl-stars/en/articles/brawler-classes.html?utm_source=chatgpt.com "Brawler Classes | Supercell Support Portal"
[2]: https://smashlegends.com/en/announcement-regarding-answer-for-dev-qna-to-celebrate-250k-discord-members/?utm_source=chatgpt.com "SMASH LEGENDS | Announcement regarding Answer for Dev QnA to celebrate 250K Discord Members"
