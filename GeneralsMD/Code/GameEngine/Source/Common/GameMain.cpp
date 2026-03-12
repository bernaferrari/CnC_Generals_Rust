// GameMain.cpp
// The main entry point for the game
// Author: Michael S. Booth, April 2001

#include "PreRTS.h"	// This must go first in EVERY cpp file in the GameEngine

#include "Common/GameEngine.h"


/**
 * This is the entry point for the game system.
 */
void GameMain( int argc, char *argv[] )
{
	// initialize the game engine using factory function
	TheGameEngine = CreateGameEngine();
	TheGameEngine->init(argc, argv);

	// run it
	TheGameEngine->execute();

	// since execute() returned, we are exiting the game
	delete TheGameEngine;
	TheGameEngine = NULL;

}

