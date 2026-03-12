// FILE: SpecialPowerCreate.h /////////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, May 2002
// Desc:   When a building is created, tell special powers to start counting down
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _SPECIAL_POWER_CREATE_H_
#define _SPECIAL_POWER_CREATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/CreateModule.h"

class Thing;

//-------------------------------------------------------------------------------------------------
/** SpecialPowerCreate */
//-------------------------------------------------------------------------------------------------
class SpecialPowerCreate : public CreateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SpecialPowerCreate, "SpecialPowerCreate" )
	MAKE_STANDARD_MODULE_MACRO( SpecialPowerCreate )

public:

	SpecialPowerCreate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onCreate( void );
	virtual void onBuildComplete();	///< This is called when you are a finished game object

protected:

};

#endif // _SPECIAL_POWER_CREATE_H_
