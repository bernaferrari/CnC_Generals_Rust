// FILE: SupplyWarehouseDockUpdate.h /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood Feb 2002
// Desc:   The action of this dock update is identifying who is docking and either taking Boxes away or giving them
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _SUPPLY_WAREHOUSE_DOCK_UPDATE_H_
#define _SUPPLY_WAREHOUSE_DOCK_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/INI.h"
#include "Common/GameMemory.h"
#include "GameLogic/Module/DockUpdate.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class SupplyWarehouseDockUpdateModuleData : public DockUpdateModuleData
{
public:

  SupplyWarehouseDockUpdateModuleData( void );
	
	static void buildFieldParse(MultiIniFieldParse& p);

	Int m_startingBoxesData;
	Bool m_deleteWhenEmpty;
};

//-------------------------------------------------------------------------------------------------
class SupplyWarehouseDockUpdate : public DockUpdate
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SupplyWarehouseDockUpdate, "SupplyWarehouseDockUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SupplyWarehouseDockUpdate, SupplyWarehouseDockUpdateModuleData )

public:

	virtual DockUpdateInterface* getDockUpdateInterface() { return this; }

	SupplyWarehouseDockUpdate( Thing *thing, const ModuleData* moduleData );

	virtual void setDockCrippled( Bool setting ); ///< Game Logic can set me as inoperative.  I get to decide what that means.
	virtual Bool action( Object* docker, Object *drone = NULL );	///<For me, this means identifying who is docking and either taking Boxes away or giving them

	Int getBoxesStored() const { return m_boxesStored; }

	void setCashValue( Int cashValue );

	virtual void onObjectCreated();
protected:


	Int m_boxesStored;

};

#endif
