// FILE: BeaconClientUpdate.h ////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, August 2002
// Desc:   Beacon client update module
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __BEACONCLIENTUPDATE_H_
#define __BEACONCLIENTUPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/ClientUpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class ParticleSystem;

//-------------------------------------------------------------------------------------------------
class BeaconClientUpdateModuleData : public ClientUpdateModuleData
{
public:
	UnsignedInt m_framesBetweenRadarPulses;
	UnsignedInt m_radarPulseDuration;

	BeaconClientUpdateModuleData();
	~BeaconClientUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
/** The tree way client update module */
//-------------------------------------------------------------------------------------------------
class BeaconClientUpdate : public ClientUpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( BeaconClientUpdate, "BeaconClientUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( BeaconClientUpdate, BeaconClientUpdateModuleData );

public:

	BeaconClientUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// the client update callback
	virtual void clientUpdate( void );
	void hideBeacon( void );

protected:

	ParticleSystemID m_particleSystemID;
	UnsignedInt m_lastRadarPulse;

};

#endif // __BEACONCLIENTUPDATE_H_

