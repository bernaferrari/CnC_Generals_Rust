#include "filteredsound.h"
#include "wwaudio.h"
#include "soundscene.h"
#include "soundchunkids.h"
#include "persistfactory.h"
#include "soundhandle.h"


//////////////////////////////////////////////////////////////////////////////////
//
//	Static factories
//
//////////////////////////////////////////////////////////////////////////////////
SimplePersistFactoryClass<FilteredSoundClass, CHUNKID_FILTERED_SOUND> _FilteredSoundPersistFactory;


/////////////////////////////////////////////////////////////////////////////////
//
//	FilteredSoundClass
//
/////////////////////////////////////////////////////////////////////////////////
FilteredSoundClass::FilteredSoundClass (void)
	:	m_hFilter (INVALID_MILES_HANDLE)
{	
	return ;
}


////////////////////////////////////////////////////////////////////////////////////////////////
//
//	FilteredSoundClass
//
////////////////////////////////////////////////////////////////////////////////////////////////
FilteredSoundClass::FilteredSoundClass (const FilteredSoundClass &src)
	:	m_hFilter (INVALID_MILES_HANDLE),
		SoundPseudo3DClass (src)
{
	(*this) = src;
	return ;
}


/////////////////////////////////////////////////////////////////////////////////
//
//	~FilteredSoundClass
//
/////////////////////////////////////////////////////////////////////////////////
FilteredSoundClass::~FilteredSoundClass (void)
{	
	return ;
}


////////////////////////////////////////////////////////////////////////////////////////////////
//
//	operator=
//
////////////////////////////////////////////////////////////////////////////////////////////////
const FilteredSoundClass &
FilteredSoundClass::operator= (const FilteredSoundClass &src)
{
	// Call the base class
	SoundPseudo3DClass::operator= (src);
	return (*this);
}


/////////////////////////////////////////////////////////////////////////////////
//
//	Initialize_Miles_Handle
//
/////////////////////////////////////////////////////////////////////////////////
void
FilteredSoundClass::Initialize_Miles_Handle (void)
{
	SoundPseudo3DClass::Initialize_Miles_Handle ();
	m_hFilter = WWAudioClass::Get_Instance ()->Get_Reverb_Filter ();
	if ((m_SoundHandle != NULL) &&
		 (m_hFilter != INVALID_MILES_HANDLE)) {
		
		//
		//	Pass the filter onto the sample
		//
		::AIL_set_sample_processor (m_SoundHandle->Get_HSAMPLE (), DP_FILTER, m_hFilter);

		//
		//	Change the reverb's settings to simulate a 'tinny' effect.
		//
		F32 reverb_level   = 0.3F;
		F32 reverb_reflect = 0.01F;
		F32 reverb_decay   = 0.535F;
		::AIL_set_filter_sample_preference (m_SoundHandle->Get_HSAMPLE (),
														"Reverb level",
														&reverb_level);

		::AIL_set_filter_sample_preference (m_SoundHandle->Get_HSAMPLE (),
														"Reverb reflect time",
														&reverb_reflect);

		::AIL_set_filter_sample_preference (m_SoundHandle->Get_HSAMPLE (),
														"Reverb decay time",
														&reverb_decay);
	}
	
	Update_Volume ();
	return ;
}


/////////////////////////////////////////////////////////////////////////////////
//
//	Update_Volume
//
/////////////////////////////////////////////////////////////////////////////////
void
FilteredSoundClass::Update_Volume (void)
{
	if (m_SoundHandle != NULL) {

		// Determine the listener's position and the sound's position
		SoundSceneClass *scene = WWAudioClass::Get_Instance ()->Get_Sound_Scene ();
		Listener3DClass *listener = scene->Peek_2nd_Listener ();
		if (listener != NULL) {
			Vector3 listener_pos = listener->Get_Position ();
			Vector3 sound_pos = m_Transform.Get_Translation ();

			// Determine a normalized volume from the position
			float distance = (sound_pos - listener_pos).Quick_Length ();
			Update_Pseudo_Volume (distance);
		}
	}

	return ;
}


/////////////////////////////////////////////////////////////////////////////////
//
//	Get_Factory
//
/////////////////////////////////////////////////////////////////////////////////
const PersistFactoryClass &
FilteredSoundClass::Get_Factory (void) const
{
	return _FilteredSoundPersistFactory;
}

