/******************************************************************************
*
* FILE
*     $Archive: /Renegade Setup/Autorun/CallbackHook.h $
*
* DESCRIPTION
*
* PROGRAMMER
*     Steven Clinard
*     $Author: Maria_l $
*
* VERSION INFO
*     $Modtime: 8/14/00 7:52p $
*     $Revision: 2 $
*
******************************************************************************/

#ifndef CALLBACKHOOK_H
#define CALLBACKHOOK_H

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


template<class T> class Callback : public CallbackHook
	{
	public:
		Callback(bool (*callback)(T), T userdata)
			: mCallback(callback),
			  mUserData(userdata)
			{
			}

		virtual ~Callback()
			{
			}

		virtual bool DoCallback(void)
			{
			if (mCallback != NULL)
				{
				return mCallback(mUserData);
				}

			return false;
			}

	private:
		bool (*mCallback)(T);
		T mUserData;
	};

#endif // CALLBACKHOOK_H
