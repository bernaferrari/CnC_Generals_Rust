// FILE: DamDie.h /////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, April 2002
// Desc:   The big water dam dying
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DAMDIE_H_
#define __DAMDIE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/DieModule.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class DamDieModuleData : public DieModuleData
{

public:

	DamDieModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);
		
};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class DamDie : public DieModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( DamDie, "DamDie" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( DamDie, DamDieModuleData )

public:

	DamDie( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prorotype provided by MemoryPoolObject

	virtual void onDie( const DamageInfo *damageInfo ); 

};

#endif  // end __DAMDIE_H_
