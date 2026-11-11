# Survival: health, hunger, death

The first, simple version (ADR 0040); both grow deeper later (food, air, temperature, armour).

- **Health**, 0 to 20 points, ten segments. It falls with an **impact**: the speed the body loses when it hits
  something solid, in any direction. Up to 7.7 m/s is free (a 3 m fall on Terra), then a point per metre of
  such a fall; 21 m/s kills from full. A slow landing on Luna costs little or nothing; a fast hit anywhere
  kills. Water breaks a fall.
- **Hunger**, 0 to 20, drains over one game day standing, twice as fast walking, and a little per block dug.
  It follows the player's own steps, so a time warp does not starve anyone. Above 14 health comes back
  (a point per 8 s); at 0 health drains (a point per 10 s) down to 1: until there is food, hunger alone
  never kills.
- **Death** at 0 health: the screen dims, says how, and offers Respawn (Terra's spawn, full health and
  hunger, the inventory kept) or the title. Everyone reads how in the chat.
- **Creative** has neither, and shows no bars: the bars over the hotbar are how survival is told from creative.
- Until food exists: `/heal`, `/feed [points]`, `/hurt <points>`.

Open: what dying costs once there is something to lose; food, cooking and how hunger feels; whether creative
flight should ever be hurt.
