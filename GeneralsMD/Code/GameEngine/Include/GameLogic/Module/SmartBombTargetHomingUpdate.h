// FILE: SmartBombTargetHomingUpdate.h /////////////////////////////////////////////////////////////////////////
// Author: Mark Lorenzen, July 2003
// Desc:   Update that will fudge a falling object's position just slightly, to make it find its target better
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SMARTBOMB_UPDATE_H_
#define __SMARTBOMB_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

//-------------------------------------------------------------------------------------------------
class SmartBombTargetHomingUpdateModuleData : public UpdateModuleData
{
public:
	Real m_courseCorrectionScalar;

	SmartBombTargetHomingUpdateModuleData()
	{
		m_courseCorrectionScalar = 0.99f;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "CourseCorrectionScalar",	INI::parseReal,		NULL, offsetof( SmartBombTargetHomingUpdateModuleData, m_courseCorrectionScalar ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class SmartBombTargetHomingUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SmartBombTargetHomingUpdate, "SmartBombTargetHomingUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SmartBombTargetHomingUpdate, SmartBombTargetHomingUpdateModuleData )

public:

	SmartBombTargetHomingUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

  void SetTargetPosition( const Coord3D& target );

	virtual UpdateSleepTime update( void );

protected:


  Bool      m_targetReceived;
  Coord3D   m_target;


};

#endif // __SMARTBOMB_UPDATE_H_

