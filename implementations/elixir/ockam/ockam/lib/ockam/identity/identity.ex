defmodule Ockam.Identity do
  @moduledoc """
  API facade for identity implementations
  Using module name and opaque data to represent implementation-specific identities

  You can chose an implementation when creating an identity

  Default implementation is `Ockam.Identity.Sidecar`
  """

  @type t() :: {module :: atom, opaque :: any()}
  @type proof() :: binary()

  @default_implementation Ockam.Identity.Sidecar

  def default_implementation() do
    @default_implementation
  end

  @spec create(module :: atom()) ::
          {:ok, identity :: t(), identity_id :: binary()} | {:error, reason :: any()}
  def create(module \\ @default_implementation) do
    with {:ok, data, id} <- module.create() do
      {:ok, {module, data}, id}
    end
  end

  @spec validate_data(my_identity :: t(), data :: any()) :: {:ok, identity :: t()}
  def validate_data({my_module, _my_data}, contact_data) do
    with {:ok, contact_id} <- validate_identity_change_history({my_module, contact_data}) do
      {:ok, {my_module, contact_data}, contact_id}
    end
  end

  @spec get_data(t()) :: any()
  def get_data({_module, data}) do
    data
  end

  @spec validate_identity_change_history(contact :: t()) ::
          {:ok, contact_id :: binary()} | {:error, reason :: any()}
  def validate_identity_change_history({module, data}) do
    module.validate_identity_change_history(data)
  end

  @spec create_signature(identity :: t(), auth_hash :: binary()) ::
          {:ok, proof :: proof()} | {:error, reason :: any()}
  def create_signature({module, data}, auth_hash) do
    module.create_signature(data, auth_hash)
  end

  @spec verify_signature(
          identity :: t(),
          proof :: proof(),
          auth_hash :: binary()
        ) :: :ok | {:error, reason :: any()}
  def verify_signature({module, data}, proof, auth_hash) do
    module.verify_signature(data, proof, auth_hash)
  end

  @spec compare_identity_change_history(current_identity :: t(), known_identity :: t) ::
          {:ok, atom()} | {:error, reason :: any()}
  def compare_identity_change_history({module, current_data}, {module, known_data}) do
    module.compare_identity_change_history(current_data, known_data)
  end

  def compare_identity_change_history(current_identity, known_identity) do
    {:error, {:different_identity_implementations, current_identity, known_identity}}
  end
end
