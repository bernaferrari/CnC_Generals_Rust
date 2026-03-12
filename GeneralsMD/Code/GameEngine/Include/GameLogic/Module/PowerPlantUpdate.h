// FILE: PowerPlantUpdate.h ////////////////////////////////////////////////////////////////////////////
// Author: Amit Kumar, August 2002
// Desc:   Updating the power plant
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __POWERPLANTUPDATE_H_
#define __POWERPLANTUPDATE_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class PowerPlantUpdateModuleData : public UpdateModuleData
{

public:

	PowerPlantUpdateModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse( p );

		static const FieldParse dataFieldParse[] = 
		{

			{ "RodsExtendTime", INI::parseDurationUnsignedInt, NULL, offsetof( PowerPlantUpdateModuleData, m_rodsExtendTime ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);

	}

  UnsignedInt m_rodsExtendTime;  ///< in frames, time it takes the rods to be built

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class PowerPlantUpdateInterface
{

public:

	virtual void extendRods( Bool extend ) = 0;

};

//-------------------------------------------------------------------------------------------------
/** The Power Plant Update module */
//-------------------------------------------------------------------------------------------------
class PowerPlantUpdate : public UpdateModule,
												 public PowerPlantUpdateInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PowerPlantUpdate, "PowerPlantUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( PowerPlantUpdate, PowerPlantUpdateModuleData );

public:

	PowerPlantUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	// interface housekeeping
	virtual PowerPlantUpdateInterface* getPowerPlantUpdateInterface() { return this; }

	void extendRods( Bool extend );									 ///< extend the rods from this object
	virtual UpdateSleepTime update( void ); ///< Here's the actual work of Upgrading

protected:

	Bool m_extended;										 ///< TRUE when extend is all done

};

#endif  // end __POWERPLANTUPDATE_H_
