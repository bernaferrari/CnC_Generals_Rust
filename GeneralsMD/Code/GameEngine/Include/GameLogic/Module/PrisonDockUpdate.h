// FILE: PrisonDockUpdate.h ///////////////////////////////////////////////////////////////////////
// Author: Colin Day, August 2002
// Desc:   Dock update for prison structures
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __PRISON_DOCK_UPDATE_H_
#define __PRISON_DOCK_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/GameMemory.h"
#include "GameLogic/Module/DockUpdate.h"

#ifdef ALLOW_SURRENDER

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class PrisonDockUpdate : public DockUpdate
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PrisonDockUpdate, "PrisonDockUpdate" )
	MAKE_STANDARD_MODULE_MACRO( PrisonDockUpdate )

public:

	PrisonDockUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by MemoryPoolObject base class
	virtual DockUpdateInterface* getDockUpdateInterface() { return this; }

	virtual Bool action( Object *docker, Object *drone = NULL );	///< for me this means do some Prison

protected:
	
};

#endif

#endif  // end __PRISON_DOCK_UPDATE_H_
