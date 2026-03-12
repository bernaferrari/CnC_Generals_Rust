#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/MiscAudio.h"
#include "Common/INI.h"

const FieldParse MiscAudio::m_fieldParseTable[] = 
{ 
	{ "RadarNotifyUnitUnderAttackSound",			INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_radarUnitUnderAttackSound ) },
	{ "RadarNotifyHarvesterUnderAttackSound",	INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_radarHarvesterUnderAttackSound ) },
	{ "RadarNotifyStructureUnderAttackSound", INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_radarStructureUnderAttackSound ) },
	{ "RadarNotifyUnderAttackSound",					INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_radarUnderAttackSound ) },
	{ "RadarNotifyInfiltrationSound",					INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_radarInfiltrationSound ) },
	{ "RadarNotifyOnlineSound",								INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_radarOnlineSound ) },
	{ "RadarNotifyOfflineSound",							INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_radarOfflineSound ) },
	{ "DefectorTimerTickSound",			  				INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_defectorTimerTickSound ) },
	{ "DefectorTimerDingSound",			  				INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_defectorTimerDingSound ) },
	{ "LockonTickSound",			  							INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_lockonTickSound ) },
	{ "AllCheerSound",												INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_allCheerSound )	},
	{ "BattleCrySound",												INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_battleCrySound )	},
	{ "GUIClickSound",												INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_guiClickSound )	},
	{ "NoCanDoSound",													INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_noCanDoSound )	},
	{ "StealthDiscoveredSound",								INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_stealthDiscoveredSound ) },
	{ "StealthNeutralizedSound",							INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_stealthNeutralizedSound ) },
	{ "MoneyDepositSound",										INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_moneyDepositSound ) },
	{ "MoneyWithdrawSound",										INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_moneyWithdrawSound ) },
	{ "BuildingDisabled",											INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_buildingDisabled ) },
	{ "BuildingReenabled",										INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_buildingReenabled ) },
	{ "VehicleDisabled",											INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_vehicleDisabled ) },
	{ "VehicleReenabled",											INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_vehicleReenabled ) },
	{ "SplatterVehiclePilotsBrain",						INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_splatterVehiclePilotsBrain ) },
	{ "TerroristInCarMoveVoice",							INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_terroristInCarMoveVoice ) },
	{ "TerroristInCarAttackVoice",						INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_terroristInCarAttackVoice ) },
	{ "TerroristInCarSelectVoice",						INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_terroristInCarSelectVoice ) },
	{ "CrateHeal",														INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_crateHeal ) },
	{ "CrateShroud",													INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_crateShroud ) },
	{ "CrateSalvage",													INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_crateSalvage ) },
	{ "CrateFreeUnit",												INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_crateFreeUnit ) },
	{ "CrateMoney",														INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_crateMoney ) },
	{ "UnitPromoted",													INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_unitPromoted ) },
	{ "RepairSparks",													INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_repairSparks ) },
	{ "SabotageShutDownBuilding",							INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_sabotageShutDownBuilding ) },
	{ "SabotageResetTimeBuilding",						INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_sabotageResetTimerBuilding ) },
  { "AircraftWheelScreech",									INI::parseAudioEventRTS, NULL, offsetof( MiscAudio, m_aircraftWheelScreech ) },
	
	{ 0, 0, 0, 0 }
};


//-------------------------------------------------------------------------------------------------
void INI::parseMiscAudio( INI *ini )
{
	ini->initFromINI(TheAudio->friend_getMiscAudio(), MiscAudio::m_fieldParseTable);
}