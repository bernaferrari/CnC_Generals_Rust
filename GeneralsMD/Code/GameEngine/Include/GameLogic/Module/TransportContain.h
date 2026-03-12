// FILE: TransportContain.h ////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, March 2002
// Desc:   Contain module for transport units.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __TransportContain_H_
#define __TransportContain_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/OpenContain.h"

//-------------------------------------------------------------------------------------------------
class TransportContainModuleData : public OpenContainModuleData
{
public:
	struct InitialPayload
	{
		AsciiString name;
		Int count;
	};

	Int								m_slotCapacity;								///< max units that can be inside us
	Real							m_exitPitchRate;
	AsciiString				m_exitBone;
	InitialPayload		m_initialPayload;
	Real							m_healthRegen;
	UnsignedInt				m_exitDelay;
	Bool							m_scatterNearbyOnExit;
	Bool							m_orientLikeContainerOnExit;
	Bool							m_keepContainerVelocityOnExit;
	Bool							m_goAggressiveOnExit;
	Bool							m_armedRidersUpgradeWeaponSet;
	Bool							m_resetMoodCheckTimeOnExit;
	Bool							m_destroyRidersWhoAreNotFreeToExit;
	Bool							m_isDelayExitInAir;

	TransportContainModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);
	static void parseInitialPayload( INI* ini, void *instance, void *store, const void* /*userData*/ );

};

//-------------------------------------------------------------------------------------------------
class TransportContain : public OpenContain
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( TransportContain, "TransportContain" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( TransportContain, TransportContainModuleData )

public:

	TransportContain( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual Bool isValidContainerFor( const Object* obj, Bool checkCapacity) const;

	virtual void onCapture( Player *oldOwner, Player *newOwner ); // have to kick everyone out on capture.
	virtual void onContaining( Object *obj, Bool wasSelected );		///< object now contains 'obj'
	virtual void onRemoving( Object *obj );			///< object no longer contains 'obj'
	virtual UpdateSleepTime update();							///< called once per frame

	virtual Bool isRiderChangeContain() const { return FALSE; }
  virtual Bool isSpecialOverlordStyleContainer() const {return FALSE;}
	
	virtual Int getContainMax( void ) const;

	virtual Int getExtraSlotsInUse( void ) { return m_extraSlotsInUse; }///< Transports have the ability to carry guys how take up more than spot.

	virtual Bool isExitBusy() const;	///< Contain style exiters are getting the ability to space out exits, so ask this before reserveDoor as a kind of no-commitment check.
	virtual ExitDoorType reserveDoorForExit( const ThingTemplate* objType, Object *specificObject );
	virtual void unreserveDoorForExit( ExitDoorType exitDoor );
	virtual Bool isDisplayedOnControlBar() const {return TRUE;}///< Does this container display its contents on the ControlBar?

protected:

	// exists primarily for TransportContain to override
	virtual void killRidersWhoAreNotFreeToExit();
	virtual Bool isSpecificRiderFreeToExit(Object* obj);
	virtual Bool isPassengerAllowedToFire( ObjectID id = INVALID_ID ) const;	///< Hey, can I shoot out of this container?

	virtual void createPayload();
	void letRidersUpgradeWeaponSet( void );

	Bool m_payloadCreated;	

private:

	Int m_extraSlotsInUse;
	UnsignedInt m_frameExitNotBusy;

};

#endif // __TransportContain_H_

