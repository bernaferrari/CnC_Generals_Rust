// FILE: SupplyWarehouseCreate.h /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood Feb 2002
// Desc:   When a Supply Warehouse is created, it needs to update all the Resource brains in all players
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _SUPPLY_WAREHOUSE_CREATE_H_
#define _SUPPLY_WAREHOUSE_CREATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/CreateModule.h"

class Thing;

//-------------------------------------------------------------------------------------------------
/** SupplyWarehouseCreate */
//-------------------------------------------------------------------------------------------------
class SupplyWarehouseCreate : public CreateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SupplyWarehouseCreate, "SupplyWarehouseCreate" )
	MAKE_STANDARD_MODULE_MACRO( SupplyWarehouseCreate )

public:

	SupplyWarehouseCreate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onCreate( void );

protected:

};

#endif

