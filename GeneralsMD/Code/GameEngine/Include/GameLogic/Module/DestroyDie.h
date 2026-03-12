// FILE: DestroyDie.h /////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, November 2001
// Desc:   Default die module
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DestroyDie_H_
#define __DestroyDie_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/DieModule.h"
#include "Common/INI.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

class DestroyDie : public DieModule
{

	MAKE_STANDARD_MODULE_MACRO( DestroyDie );
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( DestroyDie, "DestroyDie" )

public:

	DestroyDie( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onDie( const DamageInfo *damageInfo ); 

};

#endif // __DestroyDie_H_

