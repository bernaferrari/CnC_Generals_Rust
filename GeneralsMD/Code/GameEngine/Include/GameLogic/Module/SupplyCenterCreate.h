// FILE: SupplyCenterCreate.h /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood Feb 2002
// Desc:   When a Supply Center is created, it needs to update all the Resource brains in all players
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _SUPPLY_CENTER_CREATE_H_
#define _SUPPLY_CENTER_CREATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/CreateModule.h"

class Thing;

//-------------------------------------------------------------------------------------------------
/** SupplyWarehouseCreate */
//-------------------------------------------------------------------------------------------------
class SupplyCenterCreate : public CreateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SupplyCenterCreate, "SupplyCenterCreate" )
	MAKE_STANDARD_MODULE_MACRO( SupplyCenterCreate )

public:

	SupplyCenterCreate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onCreate( void );
	virtual void onBuildComplete();	///< This is called when you are a finished game object

protected:

};

#endif
