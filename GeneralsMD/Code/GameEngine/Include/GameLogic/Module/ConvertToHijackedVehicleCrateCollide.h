///////////////////////////////////////////////////////////////////////////////////////////////////
//	
// FILE: ConvertToHijackedVehicleCrateCollide.h 
// Author: Mark Lorenzen, July 2002
// Desc:   A crate (actually a terrorist - mobile crate) that makes the target vehicle switch 
//				 sides, and kills its driver
//	
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef CONVERT_TO_HIJACKED_VEHICLE_CRATE_COLLIDE_H_
#define CONVERT_TO_HIJACKED_VEHICLE_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class ConvertToHijackedVehicleCrateCollideModuleData : public CrateCollideModuleData
{
public:
	UnsignedInt m_rangeOfEffect;

	ConvertToHijackedVehicleCrateCollideModuleData()
	{
		m_rangeOfEffect = 0;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    CrateCollideModuleData::buildFieldParse(p);
	}

};

//-------------------------------------------------------------------------------------------------
class ConvertToHijackedVehicleCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ConvertToHijackedVehicleCrateCollide, "ConvertToHijackedVehicleCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ConvertToHijackedVehicleCrateCollide, ConvertToHijackedVehicleCrateCollideModuleData );

public:

	ConvertToHijackedVehicleCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	/// This allows specific vetoes to certain types of crates and their data
	virtual Bool isValidToExecute( const Object *other ) const;

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );

	virtual Bool isHijackedVehicleCrateCollide() const { return TRUE; }
};

#endif
