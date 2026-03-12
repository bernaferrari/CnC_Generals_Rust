// FILE: AnimatedParticleSysBoneClientUpdate.h //////////////////////////////////////////////////////////////////
// Author: Mark Lorenzen, October 2002
// Desc:   client update module to translate particle systems with animation
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __ANIMPARTICLESYSBONEUPDATE_H_
#define __ANIMPARTICLESYSBONEUPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/ClientUpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

class AnimatedParticleSysBoneClientUpdate : public ClientUpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( AnimatedParticleSysBoneClientUpdate, "AnimatedParticleSysBoneClientUpdate" )
	MAKE_STANDARD_MODULE_MACRO( AnimatedParticleSysBoneClientUpdate );

public:

	AnimatedParticleSysBoneClientUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// the client update callback
	virtual void clientUpdate( void );


protected:


	UnsignedInt m_life;

};

#endif // __ANIMPARTICLESYSBONEUPDATE_H_

