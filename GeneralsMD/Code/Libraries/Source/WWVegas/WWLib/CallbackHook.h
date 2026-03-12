/******************************************************************************
*
* FILE
*     $Archive: /Commando/Code/wwlib/CallbackHook.h $
*
* DESCRIPTION
*
* PROGRAMMER
*     Steven Clinard
*     $Author: Denzil_l $
*
* VERSION INFO
*     $Modtime: 7/03/01 4:59p $
*     $Revision: 1 $
*
******************************************************************************/

#ifndef __CALLBACKHOOK_H__
#define __CALLBACKHOOK_H__

class CallbackHook
	{
	public:
		CallbackHook()
			{}

		virtual ~CallbackHook()
			{}
		
		virtual bool DoCallback(void)
			{return false;}

	protected:
		CallbackHook(const CallbackHook&);
		const CallbackHook& operator=(const CallbackHook&);
	};


template<class T> class Callback :
		public CallbackHook
	{
	public:
		Callback(bool (*callback)(T), T userdata) :
				mCallback(callback),
			  mUserData(userdata)
			{}

		virtual ~Callback()
			{}

		virtual bool DoCallback(void)
			{
			if (mCallback)
				{
				return mCallback(mUserData);
				}

			return false;
			}

	private:
		bool (*mCallback)(T);
		T mUserData;
	};

#endif // __CALLBACKHOOK_H__
