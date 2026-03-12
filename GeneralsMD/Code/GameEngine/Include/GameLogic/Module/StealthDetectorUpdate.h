// FILE: StealthDetectorUpdate.h //////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, May 2002
// Desc:	 An update that checks for a status bit to stealth the owning object
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __STEALTHDETECTOR_UPDATE_H_
#define __STEALTHDETECTOR_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class StealthDetectorUpdateModuleData : public UpdateModuleData
{
public:
	UnsignedInt											m_updateRate;
	Real														m_detectionRange;
	Bool														m_initiallyDisabled;
	AudioEventRTS										m_pingSound;
	AudioEventRTS										m_loudPingSound;
	const ParticleSystemTemplate*   m_IRBeaconParticleSysTmpl;
	const ParticleSystemTemplate*   m_IRParticleSysTmpl;
	const ParticleSystemTemplate*   m_IRBrightParticleSysTmpl;
	const ParticleSystemTemplate*   m_IRGridParticleSysTmpl;
	AsciiString											m_IRParticleSysBone;
	KindOfMaskType									m_extraDetectKindof;			///< units must match any kindof bits set here, in order to be detected
	KindOfMaskType									m_extraDetectKindofNot;		///< units must NOT match any kindof bits set here, in order to be detected
	Bool														m_canDetectWhileGarrisoned;
	Bool														m_canDetectWhileTransported;

	StealthDetectorUpdateModuleData()
	{
		m_updateRate = 1;
		m_detectionRange = 0.0f;
		m_initiallyDisabled = false;
		m_IRBeaconParticleSysTmpl = NULL;
		m_IRParticleSysTmpl = NULL;
		m_IRBrightParticleSysTmpl = NULL;
		m_IRGridParticleSysTmpl = NULL;
		m_extraDetectKindof.clear();
		m_extraDetectKindofNot.clear();
		m_canDetectWhileGarrisoned = false;
		m_canDetectWhileTransported = false;
	}

	static void buildFieldParse(MultiIniFieldParse& p);

};

//-------------------------------------------------------------------------------------------------
class StealthDetectorUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( StealthDetectorUpdate, "StealthDetectorUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( StealthDetectorUpdate, StealthDetectorUpdateModuleData );

public:

	StealthDetectorUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	Bool isSDEnabled() const { return m_enabled; }
	void setSDEnabled( Bool enabled );
	virtual UpdateSleepTime update();
	virtual DisabledMaskType getDisabledTypesToProcess() const { return MAKE_DISABLED_MASK( DISABLED_HELD ); }

private:
	Bool m_enabled;

};


#endif	// __STEALTHDETECTOR_UPDATE_H_

