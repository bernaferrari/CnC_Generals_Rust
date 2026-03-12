// FILE: LifetimeUpdate.h /////////////////////////////////////////////////////////////////////////
// Author: Colin Day, December 2001
// Desc:   Update that will count down a lifetime and destroy object when it reaches zero
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __LIFETIMEUPDATE_H_
#define __LIFETIMEUPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

//-------------------------------------------------------------------------------------------------
class LifetimeUpdateModuleData : public UpdateModuleData
{
public:
	UnsignedInt m_minFrames;
	UnsignedInt m_maxFrames;

	LifetimeUpdateModuleData()
	{
		m_minFrames = 0.0f;
		m_maxFrames = 0.0f;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "MinLifetime",					INI::parseDurationUnsignedInt,		NULL, offsetof( LifetimeUpdateModuleData, m_minFrames ) },
			{ "MaxLifetime",					INI::parseDurationUnsignedInt,		NULL, offsetof( LifetimeUpdateModuleData, m_maxFrames ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class LifetimeUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( LifetimeUpdate, "LifetimeUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( LifetimeUpdate, LifetimeUpdateModuleData )

public:

	LifetimeUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	void setLifetimeRange( UnsignedInt minFrames, UnsignedInt maxFrames );
	UnsignedInt getDieFrame() const { return m_dieFrame; }

	virtual UpdateSleepTime update( void );

private:

	UnsignedInt calcSleepDelay(UnsignedInt minFrames, UnsignedInt maxFrames);

	UnsignedInt m_dieFrame;
};

#endif // __LIFETIMEUPDATE_H_

