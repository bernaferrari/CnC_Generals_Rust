// FILE: KeepObjectDie.h /////////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, November 2002
// Desc:   Die module for things that want to leave rubble in the world and don't have other die
//         modules. This fixes civilian buildings that don't have garrison contains. Garrison
//         contains have a die module built in, so these buildings need something. Without it
//         they default to the destroydie module which outright removes the object.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __KEEP_OBJECT_DIE_H_
#define __KEEP_OBJECT_DIE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/DieModule.h"
#include "Common/INI.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

class KeepObjectDie : public DieModule
{

	MAKE_STANDARD_MODULE_MACRO( KeepObjectDie );
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( KeepObjectDie, "KeepObjectDie" )

public:

	KeepObjectDie( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onDie( const DamageInfo *damageInfo ); 

};

#endif // __KEEP_OBJECT_DIE_H_

