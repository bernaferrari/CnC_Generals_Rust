// FILE: W3DGameLogic.h ///////////////////////////////////////////////////////
//
// W3D game logic class, there shouldn't be a lot of new functionality
// in this class, but there are certain things that need to have close 
// knowledge of each other like ther logical and visual terrain
//
// Author: Colin Day, April 2001
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DGAMELOGIC_H_
#define __W3DGAMELOGIC_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "GameLogic/GameLogic.h"
#include "W3DDevice/GameLogic/W3DTerrainLogic.h"
#include "W3DDevice/GameLogic/W3DGhostObject.h"

class W3DGhostObjectManager;
///////////////////////////////////////////////////////////////////////////////
// PROTOTYPES /////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

//-----------------------------------------------------------------------------
// W3DGameLogic
//-----------------------------------------------------------------------------
/** W3D specific functionality for game logic */
//-----------------------------------------------------------------------------
class W3DGameLogic : public GameLogic
{

public:

protected:

	/// factory for TheTerrainLogic, called from init()
	virtual TerrainLogic *createTerrainLogic( void ) { return NEW W3DTerrainLogic; };
	virtual GhostObjectManager *createGhostObjectManager(void) { return NEW W3DGhostObjectManager; }

};  // end class W3DGameLogic

#endif  // end __W3DGAMELOGIC_H_
