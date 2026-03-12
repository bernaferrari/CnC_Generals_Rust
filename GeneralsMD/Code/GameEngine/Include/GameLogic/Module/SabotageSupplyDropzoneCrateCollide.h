///////////////////////////////////////////////////////////////////////////////////////////////////
//	
// FILE: SabotageSupplyDropzoneCrateCollide.h 
// Author: Kris Morness, June 2003
// Desc:   A crate (actually a saboteur - mobile crate) that makes the target powerplant lose power
//	
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef SABOTAGE_SUPPLY_DROPZONE_CRATE_COLLIDE_H_
#define SABOTAGE_SUPPLY_DROPZONE_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class SabotageSupplyDropzoneCrateCollideModuleData : public CrateCollideModuleData
{
public:
	UnsignedInt m_stealCashAmount;

	SabotageSupplyDropzoneCrateCollideModuleData()
	{
		m_stealCashAmount = 0;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    CrateCollideModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "StealCashAmount",	INI::parseUnsignedInt, NULL, offsetof( SabotageSupplyDropzoneCrateCollideModuleData, m_stealCashAmount ) },
			{ 0, 0, 0, 0 }
		};
		p.add( dataFieldParse );
	}

};

//-------------------------------------------------------------------------------------------------
class SabotageSupplyDropzoneCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SabotageSupplyDropzoneCrateCollide, "SabotageSupplyDropzoneCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SabotageSupplyDropzoneCrateCollide, SabotageSupplyDropzoneCrateCollideModuleData );

public:

	SabotageSupplyDropzoneCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	/// This allows specific vetoes to certain types of crates and their data
	virtual Bool isValidToExecute( const Object *other ) const;

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );

	virtual Bool isSabotageBuildingCrateCollide() const { return TRUE; }

};

#endif
