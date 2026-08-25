#pragma once

// RandomValue.cpp only references TheGameLogic from optional debug logging;
// keeping the declaration here lets the original unit compile without
// linking the Windows-only engine.
class GameLogic {};
extern GameLogic *TheGameLogic;
