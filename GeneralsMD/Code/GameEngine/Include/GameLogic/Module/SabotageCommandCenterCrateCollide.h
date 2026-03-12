///////////////////////////////////////////////////////////////////////////////////////////////////
//	
// FILE: SabotageCommandCenterCrateCollide.h 
// Author: Kris Morness, June 2003
// Desc:   A crate (actually a saboteur - mobile crate) that resets ALL command center general powers
//	
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef SABOTAGE_COMMAND_CENTER_CRATE_COLLIDE_H_
#define SABOTAGE_COMMAND_CENTER_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class SabotageCommandCenterCrateCollideModuleData : public CrateCollideModuleData
{
public:

	SabotageCommandCenterCrateCollideModuleData()
	{
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    CrateCollideModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ 0, 0, 0, 0 }
		};
		p.add( dataFieldParse );
	}

};

//-------------------------------------------------------------------------------------------------
class SabotageCommandCenterCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SabotageCommandCenterCrateCollide, "SabotageCommandCenterCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SabotageCommandCenterCrateCollide, SabotageCommandCenterCrateCollideModuleData );

public:

	SabotageCommandCenterCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	/// This allows specific vetoes to certain types of crates and their data
	virtual Bool isValidToExecute( const Object *other ) const;

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );

	virtual Bool isSabotageBuildingCrateCollide() const { return TRUE; }

};

#endif
