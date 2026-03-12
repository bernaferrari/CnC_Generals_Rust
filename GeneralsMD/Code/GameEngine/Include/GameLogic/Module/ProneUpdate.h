// FILE: ProneUpdate.h //////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2002
// Desc:   Update module to encapsulate what it means to be "prone"
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __PRONE_UPDATE_H_
#define __PRONE_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class DamageInfo;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class ProneUpdateModuleData : public ModuleData
{
public:
  Real		m_damageToFramesRatio;      ///< Conversion from damage dealt to number of frames we cower

	ProneUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);

private: 

};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class ProneUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ProneUpdate, "ProneUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ProneUpdate, ProneUpdateModuleData );

public:

	ProneUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	void goProne( const DamageInfo *damageInfo );

	virtual UpdateSleepTime update();

protected:

	void startProneEffects();
	void stopProneEffects();

	Int m_proneFrames;
};


#endif

