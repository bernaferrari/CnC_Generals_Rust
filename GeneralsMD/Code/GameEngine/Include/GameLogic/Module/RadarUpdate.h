// FILE: RadarUpdate.h ////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, April 2002
// Desc:   Updating a radar on an object
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __RADARUPDATE_H_
#define __RADARUPDATE_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class RadarUpdateModuleData : public UpdateModuleData
{

public:

	RadarUpdateModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse( p );

		static const FieldParse dataFieldParse[] = 
		{

			{ "RadarExtendTime", INI::parseDurationReal, NULL, offsetof( RadarUpdateModuleData, m_radarExtendTime ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);

	}

  Real m_radarExtendTime;  ///< in frames, time it takes the radar tower to build

};

//-------------------------------------------------------------------------------------------------
/** The Radar Update module */
//-------------------------------------------------------------------------------------------------
class RadarUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( RadarUpdate, "RadarUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( RadarUpdate, RadarUpdateModuleData );

public:

	RadarUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	void extendRadar( void );									 ///< extend the radar from this object
	Bool isRadarActive() { return m_radarActive; }

	virtual UpdateSleepTime update( void ); ///< Here's the actual work of Upgrading

protected:

	UnsignedInt m_extendDoneFrame;						 ///< radar will be done extending on this frame
	Bool m_extendComplete;										 ///< TRUE when extend is all done
	Bool m_radarActive;												 ///< TRUE when radar is actually online and generating radar information.

};

#endif  // end __RADARUPDATE_H_
