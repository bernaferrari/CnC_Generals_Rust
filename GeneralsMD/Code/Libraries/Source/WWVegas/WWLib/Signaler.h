/******************************************************************************
*
* FILE
*     $Archive: /Commando/Code/wwlib/Signaler.h $
*
* DESCRIPTION
*     Lightweight two-way notification system. This class allows loose coupling
*     communication between two classes. The only details that need to be know
*     by both classes is the Signaler class it self and the type of signal they
*     communicate to each other.
*
* PROGRAMMER
*     Denzil E. Long, Jr.
*     $Author: Denzil_l $
*
* VERSION INFO
*     $Modtime: 11/16/01 11:19a $
*     $Revision: 4 $
*
******************************************************************************/

#ifndef __SIGNALER_H__
#define __SIGNALER_H__

template<typename T> class Signaler
	{
	public:
		void SignalMe(Signaler<T>& target)
			{if (mConnection != &target) {Disconnect(); Connect(target); target.Connect(*this);}}

		void StopSignaling(Signaler<T>& target)
			{if (mConnection == &target) {Disconnect();}}

		void SendSignal(T& signal)
			{if (mConnection) {mConnection->ReceiveSignal(signal);}}

		virtual void ReceiveSignal(T&)
			{}

		virtual void SignalDropped(Signaler<T>& signaler)
			{mConnection = NULL;}

	protected:
		Signaler() :
				mConnection(NULL)
			{}
	
		virtual ~Signaler()
			{Disconnect();}

		void Connect(Signaler<T>& source)
			{mConnection = &source;}

		void Disconnect(void)
			{if (mConnection) {mConnection->SignalDropped(*this);} mConnection = NULL;}

		// Prevent copy and assignment
		Signaler(const Signaler&);
		const Signaler& operator=(const Signaler&);

	private:
		Signaler<T>* mConnection;
	};

#endif // __SIGNALER_H__


